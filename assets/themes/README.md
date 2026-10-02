# Bundled themes

Neptune bundles every `.itermcolors` scheme from
[iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes/tree/450b05ea0962456b424e5736aed0bec7fd147f28),
the collection behind <https://iterm2colorschemes.com/>. The pinned revision is
`450b05ea0962456b424e5736aed0bec7fd147f28`: 694 files, plus 18 embedded light/dark
variants, yielding 712 presets. Names are stable config identifiers prefixed
with `iterm:`; upstream presets are immutable. No downloads occur at runtime.

[LICENSE](LICENSE) contains the collection's MIT license and
[CREDITS.md](CREDITS.md) preserves upstream author attribution. Individual theme
authors retain their rights, as stated by upstream. These notices accompany
release archives. The generated data is in `src/theme_catalog.rs`.

Regenerate using a clean checkout of the pinned upstream revision:

```sh
python3 scripts/import-themes.py /path/to/iTerm2-Color-Schemes
```

The importer checks the revision, reads plist data without executing upstream
code, and records SHA-256 hashes of every input in [source.json](source.json).
It includes the 16 ANSI colors, foreground/background, bold foreground, cursor
and cursor text, selection and selected text. Missing bold/selected text use
foreground; missing cursor text uses background. Display P3 conversion and
8-bit quantization match upstream `tools/gen.py` (MIT); calibrated/unspecified
RGB follows upstream's component interpretation. Out-of-gamut P3 is clipped to
sRGB. Neptune does not implement iTerm-specific tab, badge, cursor-guide or
link decoration colors. Neptune derives window materials and readable
interface colors from the same palette, so one theme styles the whole app.
