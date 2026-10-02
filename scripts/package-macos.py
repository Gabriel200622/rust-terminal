#!/usr/bin/env python3
"""Wrap a built macOS executable in Neptune.app with its Finder/Dock icon."""

import argparse
from pathlib import Path
import plistlib
import shutil
import tomllib


REPO = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=REPO / "target/release/neptune")
    parser.add_argument("--output", type=Path, default=REPO / "target/release/Neptune.app")
    args = parser.parse_args()
    if not args.binary.is_file():
        parser.error("Build the macOS neptune binary first")
    if args.output.exists():
        parser.error("Output already exists; choose a fresh --output directory")
    contents = args.output / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    executable = contents / "MacOS/neptune"
    shutil.copy2(args.binary, executable)
    executable.chmod(executable.stat().st_mode | 0o111)
    shutil.copy2(REPO / "assets/icons/neptune.icns", contents / "Resources/neptune.icns")
    version = tomllib.loads((REPO / "Cargo.toml").read_text())["package"]["version"]
    info = {
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleIdentifier": "rs.neptune.terminal",
        "CFBundleName": "Neptune",
        "CFBundleDisplayName": "Neptune",
        "CFBundleExecutable": "neptune",
        "CFBundleIconFile": "neptune.icns",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "NSHighResolutionCapable": True,
    }
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump(info, output)
    (contents / "PkgInfo").write_bytes(b"APPL????")


if __name__ == "__main__":
    main()
