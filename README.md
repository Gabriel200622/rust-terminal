# Neptune

<img src="assets/branding/neptune-icon.png" alt="Neptune logo" width="96" height="96">

[neptune.rs](https://neptune.rs) · [GitHub](https://github.com/zevem/neptune)

A native Rust terminal for focused work. GPU rendering, real shell sessions, and a quiet workspace interface inspired by cmux and Ghostty. The interface follows a native, Apple-style visual language: a full-height sidebar, a unified toolbar, terminals as rounded content surfaces, and one accent colour for focus. See [the interface direction](docs/design.md).

Neptune uses `egui`/`eframe` with `wgpu`, `alacritty_terminal` for terminal state, and `portable-pty` for Unix PTYs and Windows ConPTY. It contains no webview. This repository is an initial implementation: Linux is the local verification platform; macOS and Windows need native runtime verification before a production release.

## Build and run

Install a pinned Rust 1.97.1 toolchain (installed automatically by rustup) with Cargo. A graphical desktop and a working graphics driver are required. On Debian/Ubuntu, install the native build dependencies:

```sh
sudo apt-get install pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
cargo run --release --locked --bin neptune
```

On macOS, install Xcode Command Line Tools. On Windows, use the MSVC Rust toolchain with the Visual Studio C++ Build Tools and Windows SDK; ConPTY requires Windows 10 version 1809 or later.

```sh
cargo build --release --locked --bin neptune
```

The executable is `target/release/neptune` on Linux/macOS and `target\release\neptune.exe` on Windows.

### Development builds

The repository defaults to two Cargo build jobs per invocation so agents working
in parallel leave capacity for the desktop. For a shared compiler cache, install
[sccache](https://github.com/mozilla/sccache#installation) using a prebuilt release
or package manager and merge these settings into your user Cargo configuration
(`~/.cargo/config.toml`, or `$CARGO_HOME/config.toml` when set):

```toml
[build]
jobs = 2
rustc-wrapper = "sccache"
```

The user-level job limit covers existing worktrees before they receive the
repository configuration. Keep existing configuration entries when merging.
The wrapper is optional and configured per user so other contributors and CI
can build without installing sccache. Worktrees share its local disk cache while
keeping separate Cargo target directories; use `sccache --show-stats` to inspect
reuse. [Incremental Rust compilations and binary crates are not cached](https://github.com/mozilla/sccache/blob/v0.17.0/docs/Rust.md).

## Workspaces and customization

Workspace navigation, independent shell panes, split layouts, scrollback, terminal search, selection/clipboard, a command palette, and settings are integrated in the native interface. Every action is listed in the command palette with its shortcut; secondary-click a terminal or a workspace for its menu, double-click a workspace to rename it, drag a workspace to reorder the sidebar, and drag the sidebar's edge to resize it. With several terminals in a workspace, drag one by its header to rearrange them: drop it on an edge of another terminal to sit beside it, on the centre to swap places, or on a workspace in the sidebar to move it there with its shell still running. Escape cancels the drag, and the command palette lists "Move terminal to …" for each other workspace. Terminal text uses bundled JetBrains Mono; interface text uses Geist. Font licenses accompany the assets.

Hold Command on macOS or Ctrl on Linux/Windows to reveal small shortcut hints in the top-right of the first nine workspace rows. The hints follow the workspace order and disappear when you release the modifier; `⇧` means Shift. Selecting a workspace with its shortcut or the palette opens its group if it is collapsed.

### Workspace groups

Secondary-click the **Workspaces** heading or empty sidebar space for **New workspace**, **New SSH workspace**, and **New workspace group**. A group is a folder containing local or SSH workspaces. Click its name to expand or collapse it; hover the row and click **+** to open a new shell inside it. Secondary-click the folder to create an SSH workspace, rename it, or remove the group. Double-click also renames.

Use **Move to group** in a workspace's menu to organize existing workspaces, or choose **Ungrouped** to move one out. Workspaces can be reordered within their group by dragging or with **Move up** and **Move down**. Drag a group by its folder row to place it at the top, between ungrouped workspaces, or beside another group; its workspaces move with it. Ungrouped workspace rows can also be dragged around groups. Escape cancels the drag, and the new order is saved on release. Group commands are also available in the palette. Collapsing or removing a group keeps its shells running; removing it leaves its workspaces ungrouped in the folder's former position. Names, membership, empty groups, mixed sidebar order, and collapsed state are saved with the workspace organization. Groups do not nest.

### Coding agent sessions

Local Unix terminals can reopen Claude Code and Codex in their original panes
when Neptune restarts, using each CLI's own saved session ID. Start `claude` or
`codex` normally. Codex asks you to review Neptune's SessionStart hook before it
can report IDs. Exiting the CLI or restarting its terminal clears the resume
reference; closing Neptune retains it. See [agent sessions](docs/agent-sessions.md)
for setup, exact-session requirements and platform limitations.

### SSH workspaces

A workspace can be connected to another machine over SSH. Every terminal in it, including new splits and restarted terminals, then opens on that host instead of in a local shell. Secondary-click a workspace and choose **Connect over SSH…** to move all of its terminals to a host, or run **New SSH workspace** from the command palette. **Disconnect from SSH** returns the workspace to local shells. Connecting or disconnecting replaces the workspace's terminals, so processes running in them stop. A terminal keeps its session when it is moved, so it can be moved only between workspaces on the same machine.

The host is an OpenSSH destination: `host`, `user@host`, an alias from `~/.ssh/config`, or `ssh://user@host:port`. Neptune runs the system `ssh` client (which must be on your `PATH`) once per terminal, so your SSH configuration, keys and agent apply, and password, passphrase and host-key prompts appear in the terminal. Neptune stores the destination and each terminal's last reported remote directory, never a credential. Each terminal is its own connection; enable `ControlMaster` in your SSH configuration to share one. The remote host needs a POSIX `sh` to launch its login shell; the `shell` setting applies to local terminals only. A terminal whose connection ends offers **Reconnect**.

The first terminal starts in the host's login directory; splitting inherits the source terminal's current remote directory. Reconnecting or reopening the app with workspace restoration enabled opens a fresh SSH shell in each terminal's last reported remote directory. Neptune adds temporary OSC 7 directory reporting to Zsh after loading your normal login configuration, without editing your dotfiles. Other shells need their own OSC 7 integration; until a shell reports a directory, splits and reconnects start in the login directory. If a remembered remote directory no longer exists, the terminal shows the failure instead of silently opening elsewhere. Disconnecting or changing hosts clears the remembered remote directories.

When no saved workspace can be restored, Neptune opens a terminal in your home directory (`~`). Use `--cwd` to choose another starting directory, or `--ssh` to open a workspace on a host. `--command` cannot be combined with `--ssh`, and a startup command is never typed into a terminal that is connecting over SSH.

New workspace opens and selects a fresh shell at `~` immediately. Rename it later by double-clicking its sidebar row or choosing Rename workspace from its menu or the command palette.

```sh
neptune --cwd /path/to/project
neptune --ssh user@host
neptune --config config.example.toml
neptune --data-root /path/to/isolated-neptune-data
neptune --command 'printf "hello\n"'
neptune --no-restore
neptune --help
```

[config.example.toml](config.example.toml) documents the supported settings: 715 themes shared by the window and terminal, custom themes, window zoom, font size and line height, scrollback limit, shell executable, cursor style/blink, sidebar width, workspace restoration, and close confirmation. Window zoom is available in Preferences under Appearance; changes there or through zoom shortcuts are saved and restored on the next launch, including with workspace restoration disabled. Settings are validated; unknown keys are rejected. Workspace restoration restores directories, split positions, and focused panes, and launches fresh shell processes; a remote workspace opens new SSH connections to its host. Recognized coding agents can resume their provider-owned conversations through saved session references; arbitrary commands and process memory are never serialized.

Open **Preferences** and choose the theme row under Appearance (or **Browse
themes** in the command palette) to pick one theme for the whole app. Neptune's
Graphite, Dusk and Light themes sit alongside all 712 palettes from the
[iTerm2 collection](https://iterm2colorschemes.com/). Search by name or filter
Dark, Light and Custom; each card previews the window and terminal, and the
catalog opens at the theme in use. Choosing a card updates existing and new
terminals, the sidebar, toolbar and dialogs at once.
The collection works offline; [its pinned source and author
credits](assets/themes/README.md) ship with Neptune. Window surfaces and readable
interface colors are derived from each imported palette.

**New theme** copies the colors in use into an editor, and **Duplicate…** in a
card's menu (the "more" button on a card, or a secondary click) starts from any
other theme. Name it and use the color wells or hex fields for the background,
text, bold text, cursor, selection and 16 ANSI colors; the preview updates as
you edit. **Save theme** saves and applies it. Leaving the editor with unsaved
changes asks before discarding them. A custom theme's menu also offers
**Edit…** and **Delete…**; deletion asks for confirmation, and deleting the
theme in use returns to Graphite. Up to 128 custom themes are saved in
`config.toml`, with names up to 64 characters. Reset to defaults retains your
saved custom themes.
Terminal programs can still override terminal colors through escape sequences.
Older settings with a separate `terminal_theme` migrate that selection to the
single `theme` setting on load; the next settings save writes the unified format.

Default storage is `~/.config/neptune` on Linux (or `$XDG_CONFIG_HOME/neptune`),
`~/Library/Application Support/rs.Neptune.neptune` on macOS, and
`%APPDATA%\Neptune\neptune\config` on Windows. On the first normal launch,
Neptune moves the previous product's entire settings directory to this location
if Neptune storage does not already exist. Saved files and recovery copies retain
their original bytes; damaged or unsupported state still receives the usual write
protection. A migration failure stops startup and preserves the original files.
Existing Neptune storage takes precedence. Explicit `--data-root` and screenshot
launches bypass migration.

Neptune also remembers the window's size and maximized state when closed. Window state is saved as `window.json` in the data directory, independently of workspace restoration; `--no-restore` and `restore_workspaces = false` only affect workspaces. Use `--size WIDTHxHEIGHT` to override the saved geometry and start with a non-maximized window. Screenshot launches use the default or explicit size and do not save window state.

## Keyboard shortcuts

Ctrl-click a web link in a terminal to open it in your default browser; on macOS, use Command-click. Hold the modifier over a link to see its underline and hand cursor. This works with printed HTTP/HTTPS URLs and OSC 8 hyperlinks, including soft-wrapped links and visible scrollback. Ordinary clicks and drags still select text; Shift keeps selecting when a TUI owns the mouse.

Use Ctrl+Shift on Linux/Windows and Command on macOS: T opens a workspace, D splits right, E splits below, W closes the focused pane, F searches, P opens commands, B toggles the sidebar, Enter zooms the focused pane to full size and back, and 1–9 select a workspace by its sidebar position. Ctrl+Tab switches workspaces. Ctrl+Shift+Left/Right/Up/Down focuses the adjacent pane on every platform, including while zoomed; at an outer edge, focus stays put. These moves are also available in the command palette. Escape cancels a terminal drag, or leaves a sheet or a focused search field; otherwise it goes to the shell, as do Tab and unmodified arrow keys. Ctrl+comma opens preferences. Ctrl+plus/minus (Command on macOS) zooms the whole app; Ctrl+equals also zooms in, and Ctrl+0 resets app zoom (Command on macOS). On keyboards where Plus requires Shift, use Ctrl+equals (Command on macOS) for app zoom. Change terminal font size in Preferences or with Ctrl+Shift+plus/minus on Linux/Windows and Command+Shift+plus/minus on macOS; Ctrl+Shift+0 (Command+Shift+0 on macOS) resets it to the default (14 pt). On macOS these font shortcuts follow the active keyboard layout's labeled +, -, and 0 keys, even when Shift produces *, _, or =, as on Latin American keyboards. Use Ctrl+Shift+C/V to copy/paste on Linux/Windows, Command+C/V on macOS. Plain Ctrl+C interrupts the shell; Shift+PageUp/PageDown scrolls history. Hold Shift to select text when a TUI owns the mouse.

The [Neptune logo and icon exports](assets/README.md) live in `assets/`. Windows
builds embed the multi-size icon in `neptune.exe`. On macOS, wrap the built
executable in an app bundle to use the icon in Finder and the Dock:

```sh
python3 scripts/package-macos.py
open target/release/Neptune.app
```

Local bundles are unsigned. Official release DMGs require Developer ID signing, Apple notarization and stapling; see [the release guide](docs/releases.md).
On Linux, install the [desktop entry](packaging/neptune.desktop) and PNG icons
in the standard application/icon locations.

For a per-user Linux install (ensure `~/.local/bin` is on your PATH):

```sh
install -Dm755 target/release/neptune ~/.local/bin/neptune
install -Dm644 packaging/neptune.desktop ~/.local/share/applications/rs.neptune.terminal.desktop
for size in 16 24 32 48 64 128 256 512 1024; do
  install -Dm644 "assets/icons/neptune-${size}.png" "$HOME/.local/share/icons/hicolor/${size}x${size}/apps/neptune.png"
done
```

## Website

The landing page and downloads for [neptune.rs](https://neptune.rs) live in [`website/`](website/README.md): a Vercel-hosted Next.js site with server-cached GitHub release discovery. Its window demo follows the app's theme and layout code. It is separate from the Cargo workspace and uses Bun.

## Verification

See [the verification record and release checklist](docs/verification.md) for test scope, native screenshots, and platform checks still required for distribution. Local development uses the narrowest meaningful affected crate/target tests; CI owns workspace-wide formatting, lint, tests and builds. Run repo-wide checks locally only when explicitly requested. The owning [agent guides](AGENTS.md) provide scoped commands.

Use the isolated native harness below for interactive review. For targeted framebuffer capture and Linux/X11 OS-input smoke testing, follow [the native verification procedure](docs/verification.md#native-application-verification), including a fresh data root and the exact owned window/inspection endpoint.

## Performance and Ghostty

PTY reading and terminal parsing happen outside the UI thread. History is bounded and the renderer consumes the visible viewport. Performance work focuses on lock duration, repaint coalescing, cached text layout, idle CPU, and sustained output. Benchmarks must use release builds. No claim of being faster than Ghostty or Alacritty is made without reproducible end-to-end measurements. See [measured performance and reproducible probes](docs/performance.md).

Ghostty's public `libghostty-vt` is a possible future engine, but it supplies terminal state and render-state extraction rather than GPU drawing. Its API is still unstable. The full Ghostty surface API is internal and tailored to its macOS application, so it is not the current cross-platform renderer. See [the architecture decision and release gates](docs/architecture.md) for primary sources, the migration plan, and the verification matrix.

## CI and artifacts

GitHub Actions runs CI on pull requests and every push to `main`, including after a PR merges. PR runs check out GitHub's test merge commit, combining the PR with its base branch when the event triggers; pushes to `main` test the resulting commit. Change classification compares PR merge commits with their first parent and pushes with the previous `main` tip, covering the entire pushed range for merge, squash, and rebase merges. After change classification, fast formatting/architecture validation, the native platform matrix, and latest-stable compatibility run in parallel; documentation/evidence-only changes skip compilation within successful jobs. Default and inspection configurations are tested on Linux, macOS, and Windows. Linux also runs Clippy and isolated X11 inspection/restoration using the same build, while a separate job checks all targets/features with latest stable Rust. CI uses an unoptimized profile without debug symbols and caches dependencies even after failed runs; performance measurements and releases use optimized builds.

CI uses read-only repository permissions and a timeout for every job. New commits cancel older CI runs for the same PR; each push to `main` runs independently. The final `Check` status succeeds only when every CI job succeeds, rejecting failed, cancelled, or skipped jobs. The `main` ruleset requires this single `Check` status from GitHub Actions.

The active `main` ruleset requires only a pull request and passing CI. It does not require approvals, resolved review threads, a merge queue, or the PR branch to be up to date before merging. CI runs again on the resulting `main` commit after the merge.

Official desktop releases are deliberate SemVer tags, never normal merges to `main`.
[CHANGELOG.md](CHANGELOG.md) owns What's New; the release workflow copies its
version section into GitHub Release notes and authenticated update metadata.
Native runners package macOS ARM64/Intel signed and notarized DMGs, a Windows x64
per-user EXE installer (currently unsigned), and Linux x64 AppImage/DEB.
All five artifacts must succeed before a draft is staged with SHA256SUMS,
Ed25519-signed update metadata and GitHub provenance attestations. Publication
is manual after review and native acceptance.

Download published stable builds at [neptune.rs/download](https://neptune.rs/download)
and prereleases at [neptune.rs/download/beta](https://neptune.rs/download/beta).
GitHub Releases hosts the binaries; the Vercel website caches and verifies release
metadata server-side and always offers all platform/architecture choices.
Preferences → Updates controls automatic checks and Stable/Beta channels.
Checks/downloads run off the UI thread; updates require signature/hash verification
and explicit download/install actions. Running shells are never silently closed
or replaced. Beta can advance to a newer stable; neither channel downgrades.

[The release guide](docs/releases.md) contains exact version/tag commands,
GitHub environment secrets, one-time Apple setup, download verification and
failure recovery. AI agents must not create/push release tags or create/publish
Neptune releases unless explicitly asked to release that version.
Packaging automation does not replace the native/platform acceptance gates in
[docs/verification.md](docs/verification.md).

Neptune is MIT licensed. Bundled fonts retain their SIL Open Font License notices; third-party dependencies and vendored development skills retain their respective licenses. See [third-party notices](THIRD-PARTY-NOTICES.md) for skill attribution and the full upstream license texts.

Developer inspection is opt-in through the `inspection` feature. `neptune-inspect` exposes the native accessibility tree and supports `screenshot`, `key`, `text`, `click`, `context` (secondary click), `double-click`, `drag`, `press`, `release`, and `resize` commands for repeatable native visual review. The normal release binary has no inspection listener. Independent launches must follow [the run-isolation rules](scripts/AGENTS.md).

Native regression runs own their process, temporary data directory, and inspection
endpoint; they never use your saved workspaces:

```sh
cargo build -p neptune-terminal --features inspection --locked --bin neptune --bin neptune-inspect
python3 scripts/native-harness.py --restore --output artifacts/native-review
```

Use `--diagnostics` for JSON counters with pane/session generations, resource
reservations, snapshot work, and frame p50/p95/p99. Terminal contents are excluded.
`python3 scripts/measure-scale.py --help` describes reproducible many-pane runs.
