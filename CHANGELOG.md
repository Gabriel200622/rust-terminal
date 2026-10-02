# Changelog

User-facing release history. Add meaningful changes under **Unreleased** as work
lands. `scripts/release.py prepare` moves them into a dated version section.
The release workflow copies that section verbatim into GitHub's What's New and
the signed desktop update manifest; do not maintain a second release-notes file.

## [Unreleased]

### What's New

## [0.1.1] - 2026-10-02

### What's New

- Download native installers for macOS Apple Silicon and Intel, Windows x64,
  and Linux x64 from neptune.rs, with checksums and build provenance.
- Choose Stable or Beta updates in Preferences. Neptune checks in the background
  and offers verified downloads without interrupting your running shells.
- Organize independent terminals into workspaces and groups, restore directories
  and layouts with fresh shells, and connect remote workspaces using OpenSSH.
- Resume supported coding-agent sessions when reopening a workspace.
- Include bundled library license notices in Linux portable packages when Ubuntu
  exposes system libraries through `/lib` aliases.

### Known limitations

- The Windows installer is unsigned while SignPath OSS setup is pending; Windows
  may show an unverified-publisher or SmartScreen warning.
- Workspace restoration starts fresh shells and SSH connections. Running commands
  and process memory are not restored.
- Kitty graphics and comprehensive complex-script shaping are not supported.
  Keypad identity and some keyboard-layout information depend on the window backend.

## [0.1.0] - 2026-10-02

### What's New

- Download native installers for macOS Apple Silicon and Intel, Windows x64,
  and Linux x64 from neptune.rs, with checksums and build provenance.
- Choose Stable or Beta updates in Preferences. Neptune checks in the background
  and offers verified downloads without interrupting your running shells.
- Organize independent terminals into workspaces and groups, restore directories
  and layouts with fresh shells, and connect remote workspaces using OpenSSH.

### Known limitations

- The Windows installer is unsigned while SignPath OSS setup is pending; Windows
  may show an unverified-publisher or SmartScreen warning.
- Workspace restoration starts fresh shells and SSH connections. Running commands
  and process memory are not restored.
- Kitty graphics and comprehensive complex-script shaping are not supported.
  Keypad identity and some keyboard-layout information depend on the window backend.
