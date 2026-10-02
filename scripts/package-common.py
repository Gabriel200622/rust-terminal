"""Release license payload shared by native installers (stdlib only)."""
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def notices(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for filename in ("LICENSE", "config.example.toml"):
        shutil.copy2(ROOT / filename, destination / filename)
    fonts = destination / "fonts"
    fonts.mkdir()
    for path in (ROOT / "assets/fonts").glob("*LICENSE*"):
        shutil.copy2(path, fonts / path.name)
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], cwd=ROOT))
    output = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        output.append(f"{package['name']} {package['version']}\nLicense: {package.get('license') or 'see license file'}\n")
        root = Path(package["manifest_path"]).parent
        paths = set()
        if package.get("license_file"):
            paths.add(root / package["license_file"])
        for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "COPYRIGHT*"):
            paths.update(root.glob(pattern))
        for path in sorted(paths):
            if path.is_file():
                output.append(f"\n--- {path.name} ---\n{path.read_text(errors='replace')}\n")
        output.append("\n" + "=" * 72 + "\n")
    (destination / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(output), encoding="utf-8")
