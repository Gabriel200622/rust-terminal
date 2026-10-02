#!/usr/bin/env python3
"""Small release primitives. Never creates a tag or publishes a release."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REPO = "zevem/neptune"
SEMVER = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?")
PLATFORMS = {"macos-arm64": "dmg", "macos-x64": "dmg", "windows-x64": "exe", "linux-x64-appimage": "AppImage", "linux-x64-deb": "deb"}


def version_info(version):
    match = SEMVER.fullmatch(version)
    if not match or any(p.isdigit() and len(p) > 1 and p[0] == "0" for p in (match[4] or "").split(".")):
        raise ValueError("Version must be strict SemVer (without a v prefix)")
    # Windows VERSIONINFO is four unsigned 16-bit numbers.
    if any(int(match[i]) > 65535 for i in (1, 2, 3)):
        raise ValueError("Version cannot be represented by Windows VERSIONINFO")
    return match, bool(match[4])


def artifact_names(version):
    version_info(version)
    return {platform: f"Neptune-{version}-{platform.removesuffix('-appimage').removesuffix('-deb')}.{extension}" for platform, extension in PLATFORMS.items()}


def notes(version, root=ROOT):
    version_info(version)
    text = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    sections = re.findall(r"^## \[([^\]]+)\](?: - (\d{4}-\d{2}-\d{2}))?\n(.*?)(?=^## |\Z)", text, re.M | re.S)
    matches = [(date, body.strip()) for name, date, body in sections if name == version]
    if len(matches) != 1:
        raise ValueError("CHANGELOG.md must contain exactly one dated section for this version")
    date, body = matches[0]
    dt.date.fromisoformat(date)
    if "### What's New" not in body or not re.search(r"^- \S", body, re.M):
        raise ValueError("Release requires meaningful What's New bullets")
    if re.search(r"\b(TODO|TBD)\b", body):
        raise ValueError("Release notes contain unfinished placeholders")
    if len(body.encode()) > 32000:
        raise ValueError("Release notes exceed updater metadata budget")
    return body + "\n"


def validate(tag, root=ROOT):
    if not tag.startswith("v"):
        raise ValueError("Release tag must start with v")
    version = tag[1:]
    _, prerelease = version_info(version)
    if tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"] != version:
        raise ValueError("Tag must match neptune-terminal's Cargo.toml version")
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    if next(p["version"] for p in lock["package"] if p["name"] == "neptune-terminal") != version:
        raise ValueError("Cargo.lock desktop version disagrees with tag")
    public_key = (root / "packaging/update-public-key.hex").read_text().strip()
    if not re.fullmatch(r"[0-9a-f]{64}", public_key):
        raise ValueError("Provision a valid Ed25519 update public key before release")
    notes(version, root)
    return {"version": version, "prerelease": str(prerelease).lower()}


def prepare(version, root=ROOT):
    version_info(version)
    changelog = (root / "CHANGELOG.md").read_text()
    if f"## [{version}]" in changelog:
        raise ValueError("Changelog already contains this version")
    if changelog.count("## [Unreleased]") != 1:
        raise ValueError("Expected exactly one Unreleased section")
    replacement = f"## [Unreleased]\n\n### What's New\n\n## [{version}] - {dt.date.today().isoformat()}"
    new_changelog = changelog.replace("## [Unreleased]", replacement, 1)
    manifest = root / "Cargo.toml"
    old_version = tomllib.loads(manifest.read_text())["package"]["version"]
    new_manifest = manifest.read_text().replace(f'version = "{old_version}"', f'version = "{version}"', 1)
    lock = root / "Cargo.lock"
    old_lock = f'name = "neptune-terminal"\nversion = "{old_version}"'
    if old_lock not in lock.read_text():
        raise ValueError("Unexpected Cargo.lock desktop package")
    new_lock = lock.read_text().replace(old_lock, f'name = "neptune-terminal"\nversion = "{version}"', 1)
    # Validate notes before writing any prepared files.
    with tempfile.TemporaryDirectory() as directory:
        preview = Path(directory)
        (preview / "CHANGELOG.md").write_text(new_changelog)
        notes(version, preview)
    manifest.write_text(new_manifest)
    lock.write_text(new_lock)
    (root / "CHANGELOG.md").write_text(new_changelog)
    print(f"Prepared {version}. Review the diff, commit through a PR, and wait for main CI. No tag created.")


def sha256(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def manifest(version, directory, commit):
    version_info(version)
    expected = artifact_names(version)
    if set(p.name for p in directory.iterdir()) != set(expected.values()):
        raise ValueError("Refuse incomplete or unexpected release artifact set")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Expected full source commit SHA")
    assets = []
    for platform, name in expected.items():
        path = directory / name
        if not path.is_file() or not 0 < path.stat().st_size <= 1024 * 1024 * 1024:
            raise ValueError(f"Invalid artifact size: {name}")
        assets.append({"platform": platform, "name": name, "size": path.stat().st_size, "sha256": sha256(path)})
    payload = {"schema": 1, "repository": REPO, "version": version, "tag": f"v{version}", "commit": commit, "notes": notes(version), "assets": assets}
    manifest_path = directory / "update-manifest.json"
    manifest_path.write_text(json.dumps(payload, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n", encoding="utf-8")
    # Secret comes from the protected GitHub environment, never command arguments/logs.
    private = os.environ.get("UPDATE_SIGNING_KEY", "")
    if not private:
        raise ValueError("Missing UPDATE_SIGNING_KEY; no unsigned update metadata allowed")
    with tempfile.TemporaryDirectory() as temporary:
        key = Path(temporary) / "key.pem"
        key.write_text(private)
        key.chmod(0o600)
        public = subprocess.check_output(["openssl", "pkey", "-in", str(key), "-pubout", "-outform", "DER"], stderr=subprocess.DEVNULL)
        expected_key = bytes.fromhex((ROOT / "packaging/update-public-key.hex").read_text().strip())
        if public != bytes.fromhex("302a300506032b6570032100") + expected_key:
            raise ValueError("Signing key does not match embedded updater public key")
        signature = Path(temporary) / "signature"
        subprocess.run(["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", str(key), "-in", str(manifest_path), "-out", str(signature)], check=True, stderr=subprocess.DEVNULL)
        (directory / "update-manifest.sig").write_text(signature.read_bytes().hex() + "\n")
    files = sorted(directory.iterdir())
    (directory / "SHA256SUMS").write_text("".join(f"{sha256(p)}  {p.name}\n" for p in files))


def release_for_tag(tag):
    """Find drafts as well as published releases; the by-tag API excludes drafts."""
    page = 1
    found = None
    while True:
        result = subprocess.run(["gh", "api", f"repos/{REPO}/releases?per_page=100&page={page}"], capture_output=True, text=True)
        if result.returncode:
            raise ValueError("Cannot safely determine existing release state")
        releases = json.loads(result.stdout)
        if not isinstance(releases, list):
            raise ValueError("Invalid release listing")
        for release in releases:
            if release["tag_name"] == tag:
                if found is not None:
                    raise ValueError("Ambiguous releases for the same tag")
                found = release
        if len(releases) < 100:
            return found
        page += 1


def stage(tag, directory):
    """Never mutate a published release, including on workflow reruns."""
    validation = validate(tag)
    release = release_for_tag(tag)
    if release is not None:
        if not release["draft"]:
            raise ValueError("Refuse to change an already published release")
        subprocess.run(["gh", "release", "edit", tag, "--repo", REPO, "--draft", "--prerelease=" + validation["prerelease"], "--title", f"INCOMPLETE Neptune {validation['version']}", "--notes", "Incomplete draft. Wait for the Desktop release workflow to succeed before review/publication."], check=True)
        # A failed upload remains private. Remove stale artifacts before replacing.
        for asset in release["assets"]:
            subprocess.run(["gh", "api", "--method", "DELETE", f"repos/{REPO}/releases/assets/{asset['id']}"], check=True)
    else:
        subprocess.run(["gh", "release", "create", tag, "--repo", REPO, "--verify-tag", "--draft", "--prerelease=" + validation["prerelease"], "--title", f"INCOMPLETE Neptune {validation['version']}", "--notes", "Incomplete draft. Wait for the Desktop release workflow to succeed before review/publication."], check=True)
    subprocess.run(["gh", "release", "upload", tag, "--repo", REPO, *[str(p) for p in sorted(directory.iterdir())]], check=True)
    release = release_for_tag(tag)
    if release is None or not release["draft"] or {a["name"] for a in release["assets"]} != {p.name for p in directory.iterdir()}:
        raise ValueError("Draft upload verification failed; do not publish")
    subprocess.run(["gh", "release", "edit", tag, "--repo", REPO, "--draft", "--prerelease=" + validation["prerelease"], "--title", f"Neptune {validation['version']}", "--notes-file", str(directory.parent / "release-notes.md")], check=True)
    print("Complete draft uploaded. Review artifacts, provenance and native acceptance before publishing.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("prepare").add_argument("version")
    check = sub.add_parser("validate")
    check.add_argument("tag")
    check.add_argument("--github-output", type=Path)
    check.add_argument("--notes", type=Path)
    seal = sub.add_parser("manifest")
    seal.add_argument("version")
    seal.add_argument("directory", type=Path)
    seal.add_argument("--commit", required=True)
    draft = sub.add_parser("stage")
    draft.add_argument("tag")
    draft.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.version)
    elif args.command == "validate":
        result = validate(args.tag)
        if args.github_output:
            with args.github_output.open("a") as output:
                output.write("".join(f"{k}={v}\n" for k, v in result.items()))
        if args.notes:
            args.notes.write_text(notes(result["version"]))
        print(json.dumps(result))
    elif args.command == "manifest":
        manifest(args.version, args.directory, args.commit)
    else:
        stage(args.tag, args.directory)


if __name__ == "__main__":
    main()
