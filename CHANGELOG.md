# Changelog

User-facing release history. Add meaningful changes under **Unreleased** as work
lands. `scripts/release.py prepare` moves them into a dated version section.
The release workflow copies that section verbatim into GitHub's What's New and
the signed desktop update manifest; do not maintain a second release-notes file.
Earlier private drafts were withdrawn before Neptune's first public preview;
their preparation records remain in Git history.

## [Unreleased]

### What's New

- Avoid multi-second GPU presentation stalls observed after idle on Linux with
  NVIDIA graphics under XWayland. The reported intermittent native Wayland
  freeze still needs a capture of the blocking state.

## [0.1.0-rc.1] - 2026-10-02

### What's New

- Meet Neptune's first public preview: a native Rust terminal with GPU rendering
  and independent shell sessions, without a browser or webview.
- Organize terminals into workspaces and groups, split and rearrange panes,
  search scrollback, and find actions in the command palette.
- Reopen saved layouts and directories with fresh shells. Close confirmations
  help protect running work, including terminals in hidden workspaces.
- Connect entire workspaces through your system's OpenSSH client. New splits
  and restored connections use each terminal's reported remote directory.
- Choose from 715 bundled themes, save favorites, and create custom palettes.
  Adjust terminal typography and save your preferred window zoom and size.
- Pick installed shells in Preferences, including PowerShell, WSL, Git Bash and
  Windows Terminal profiles on Windows, or use a custom program.
- Receive terminal alerts through OSC 9, 99 and 777, with pane highlights,
  workspace badges, a notification popover and optional desktop banners.
- Reopen supported Claude Code and Codex conversations in their original local
  Unix panes using the providers' saved session IDs.
- Choose Stable or Beta updates and download verified installers while your
  shells keep running. Native packages include checksums, signed update metadata
  and GitHub build attestations.
- Include fixes for macOS Dock icon sizing, recovery after minimizing and zsh
  history persistence, plus Windows 11 window corners and terminal directories
  when splitting or restoring workspaces.

### Known limitations

- This is a public release candidate. Full native acceptance on every platform
  remains pending; it is not a production-stable release.
- Windows installers are unsigned. Windows may show an unverified-publisher or
  SmartScreen warning.
- Linux x64 packages target glibc 2.35 or newer and need working host graphics
  drivers. Browser-downloaded AppImages need execute permission before launch;
  enable it in file Properties or run `chmod u+x` on the downloaded file.
- Workspace restoration starts fresh shells and SSH connections. Arbitrary
  running commands and process memory are not restored. Coding-agent resumption
  is limited to supported provider sessions on local Unix terminals.
- SSH requires an installed OpenSSH client and a POSIX remote shell. Remote
  directory tracking is integrated for zsh; other shells need OSC 7 integration.
- Kitty graphics and comprehensive complex-script shaping are not supported.
  Keypad identity and some keyboard-layout information depend on the window
  toolkit. IME, accessibility and mixed-DPI behavior still need native acceptance.
- Release candidates are offered through the Beta channel; Stable waits for an
  accepted stable release. If you installed a withdrawn higher-version private
  draft, install this RC manually: automatic updates never downgrade.
