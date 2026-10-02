# Assets

Project artwork, designs and bundled fonts live here.

- [`branding/neptune-logo.png`](branding/neptune-logo.png) is the original supplied
  Neptune logo, including its transparent background and padding.
- `icons/` contains exports of that exact artwork: PNGs from 16 to 1024 pixels,
  a Windows ICO with sizes through 256 pixels, and a macOS ICNS with Retina sizes
  through 1024 pixels.
- [`fonts/`](fonts/README.md) contains the bundled typefaces and their licenses.

Regenerate the icons and website copies from the repository root:

```sh
python3 -m pip install Pillow
python3 scripts/export-logo.py
```

The native window loads `icons/neptune-256.png`; Windows builds embed
`icons/neptune.ico` in the executable. The macOS bundle uses `icons/neptune.icns`;
see [`package-macos.py`](../scripts/package-macos.py). Linux installs the PNGs in
the `hicolor` icon theme under the name `neptune`.

The website copies serve its navigation, footer, favicon, touch icon and sharing
metadata. The logo belongs to app identity and external branding; it is not
drawn inside the terminal interface or its website demo.
