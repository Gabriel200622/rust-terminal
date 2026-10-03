# Changelog

User-facing release history. Add meaningful changes under **Unreleased** as work
lands. `scripts/release.py prepare` moves them into a dated version section.
The release workflow copies that section verbatim into GitHub's What's New and
the signed desktop update manifest; do not maintain a second release-notes file.

## [Unreleased]

### What's New

- Keep working after minimizing on macOS. A window minimized with the yellow
  button no longer comes back from the Dock frozen.
- Receive terminal alerts while Neptune is minimized or covered, and confirm
  before a hidden window closes running terminals.
- See clean window corners on Windows 11. The window follows the system's own
  rounded corners and border instead of leaving gaps inside them.
- Keep zsh command history between Neptune sessions on macOS. Commands are
  saved to your usual `~/.zsh_history` again, so history search and
  autosuggestions remember them in new terminals and SSH sessions.

## [0.1.3-rc.1] - 2026-10-02

### What's New

- Keep the macOS Dock icon at the same size while Neptune is running and closed.
- Follow the Linux AppImage launch instructions to enable execution after a
  browser download, including when using Gear Lever.
- See which terminal needs you. Programs that send OSC 9, 99 or 777 alerts ring
  their pane, show the latest alert and an unread count on their workspace, and
  collect in a notification popover that opens the right terminal. Desktop
  banners are optional.
- Use native installers for macOS Apple Silicon and Intel, Windows x64, and
  Linux x64, with checksums and build provenance.
- Choose Stable or Beta updates in Preferences. Neptune checks in the background
  and offers verified downloads without interrupting your running shells.
- Organize independent terminals into workspaces and groups, restore directories
  and layouts with fresh shells, and connect remote workspaces using OpenSSH.
- Resume supported coding-agent sessions when reopening a workspace.

### Known limitations

- This release candidate is for acceptance testing. Native platform acceptance
  remains pending before a stable release can be prepared.
- Browser downloads of Linux AppImages require execute permission before launch.
- The Windows installer is unsigned while SignPath OSS setup is pending; Windows
  may show an unverified-publisher or SmartScreen warning.
- Workspace restoration starts fresh shells and SSH connections. Running commands
  and process memory are not restored.
- Kitty graphics and comprehensive complex-script shaping are not supported.
  Keypad identity and some keyboard-layout information depend on the window backend.

## [0.1.2] - 2026-10-02

### What's New

- See which terminal needs you. Programs that send OSC 9, 99 or 777 alerts ring
  their pane, show the latest alert and an unread count on their workspace, and
  collect in a notification popover that opens the right terminal. Desktop
  banners are optional.

- Download native installers for macOS Apple Silicon and Intel, Windows x64,
  and Linux x64 from neptune.rs, with checksums and build provenance.
- Choose Stable or Beta updates in Preferences. Neptune checks in the background
  and offers verified downloads without interrupting your running shells.
- Organize independent terminals into workspaces and groups, restore directories
  and layouts with fresh shells, and connect remote workspaces using OpenSSH.
- Resume supported coding-agent sessions when reopening a workspace.
- Include bundled library license notices in Linux portable packages when Ubuntu
  exposes system libraries through `/lib` aliases.

- Verify complete private release drafts without requiring publication.

### Known limitations

- The Windows installer is unsigned while SignPath OSS setup is pending; Windows
  may show an unverified-publisher or SmartScreen warning.
- Workspace restoration starts fresh shells and SSH connections. Running commands
  and process memory are not restored.
- Kitty graphics and comprehensive complex-script shaping are not supported.
  Keypad identity and some keyboard-layout information depend on the window backend.

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
