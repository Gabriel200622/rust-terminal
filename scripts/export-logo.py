#!/usr/bin/env python3
"""Export Neptune's supplied logo to native and website icon formats.

Requires Pillow (python3 -m pip install Pillow). The source's artwork,
transparent background and padding are preserved; only resolution changes.
"""

from pathlib import Path
import shutil

from PIL import Image


REPO = Path(__file__).resolve().parents[1]


def main() -> None:
    source = Image.open(REPO / "assets/branding/neptune-logo.png").convert("RGBA")
    icons = REPO / "assets/icons"
    icons.mkdir(parents=True, exist_ok=True)
    sizes = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
    for size in sizes:
        source.resize((size, size), Image.Resampling.LANCZOS).save(
            icons / f"neptune-{size}.png", optimize=True
        )
    source.save(icons / "neptune.ico", sizes=[(s, s) for s in sizes if s <= 256])
    source.save(icons / "neptune.icns")

    public = REPO / "website/public"
    public.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(icons / "neptune-256.png", public / "neptune-logo.png")
    app = REPO / "website/src/app"
    shutil.copyfile(icons / "neptune-64.png", app / "icon.png")
    source.save(app / "favicon.ico", sizes=[(16, 16), (32, 32), (48, 48)])
    # Web clips need an opaque background; the logo itself remains unchanged.
    touch = Image.new("RGB", (180, 180), "#101012")
    small = source.resize((180, 180), Image.Resampling.LANCZOS)
    touch.paste(small, mask=small.getchannel("A"))
    touch.save(app / "apple-icon.png", optimize=True)


if __name__ == "__main__":
    main()
