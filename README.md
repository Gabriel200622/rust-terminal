# Pace

A native Rust terminal for focused work. GPU rendering, real shell sessions, and a quiet workspace interface inspired by cmux and Ghostty. The interface follows a native, Apple-style visual language: a full-height sidebar, a unified toolbar, terminals as rounded content surfaces, and one accent colour for focus. See [the interface direction](docs/design.md).

Pace uses `egui`/`eframe` with `wgpu`, `alacritty_terminal` for terminal state, and `portable-pty` for Unix PTYs and Windows ConPTY. It contains no webview. This repository is an initial implementation: Linux is the local verification platform; macOS and Windows need native runtime verification before a production release.

## Build and run

Install a pinned Rust 1.97.1 toolchain (installed automatically by rustup) with Cargo. A graphical desktop and a working graphics driver are required. On Debian/Ubuntu, install the native build dependencies:

```sh
sudo apt-get install pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
cargo run --release --locked --bin pace
```

On macOS, install Xcode Command Line Tools. On Windows, use the MSVC Rust toolchain with the Visual Studio C++ Build Tools and Windows SDK; ConPTY requires Windows 10 version 1809 or later.

```sh
cargo build --release --locked --bin pace
```

The executable is `target/release/pace` on Linux/macOS and `target\release\pace.exe` on Windows.

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

Hold Command on macOS or Ctrl on Linux/Windows to reveal small shortcut hints in the top-right of the first nine workspace rows. The hints follow the current sidebar order and disappear when you release the modifier; `⇧` means Shift.

### SSH workspaces

A workspace can be connected to another machine over SSH. Every terminal in it, including new splits and restarted terminals, then opens on that host instead of in a local shell. Secondary-click a workspace and choose **Connect over SSH…** to move all of its terminals to a host, or run **New SSH workspace** from the command palette. **Disconnect from SSH** returns the workspace to local shells. Connecting or disconnecting replaces the workspace's terminals, so processes running in them stop. A terminal keeps its session when it is moved, so it can be moved only between workspaces on the same machine.

The host is an OpenSSH destination: `host`, `user@host`, an alias from `~/.ssh/config`, or `ssh://user@host:port`. Pace runs the system `ssh` client (which must be on your `PATH`) once per terminal, so your SSH configuration, keys and agent apply, and password, passphrase and host-key prompts appear in the terminal. Pace stores only the destination, never a credential. Each terminal is its own connection; enable `ControlMaster` in your SSH configuration to share one. The remote host needs a POSIX `sh` to launch its login shell; the `shell` setting applies to local terminals only. A terminal whose connection ends offers **Reconnect**.

The first terminal starts in the host's login directory; splitting inherits the source terminal's current remote directory. Pace adds temporary OSC 7 directory reporting to Zsh after loading your normal login configuration, without editing your dotfiles. Other shells need their own OSC 7 integration; until a shell reports a directory, splits start in the login directory. Reconnecting or restoring opens a fresh login shell; remote directories are not saved as local paths.

When no saved workspace can be restored, Pace opens a terminal in your home directory (`~`). Use `--cwd` to choose another starting directory, or `--ssh` to open a workspace on a host. `--command` cannot be combined with `--ssh`, and a startup command is never typed into a terminal that is connecting over SSH.

New workspace opens and selects a fresh shell at `~` immediately. Rename it later by double-clicking its sidebar row or choosing Rename workspace from its menu or the command palette.

```sh
pace --cwd /path/to/project
pace --ssh user@host
pace --config config.example.toml
pace --data-root /path/to/isolated-pace-data
pace --command 'printf "hello\n"'
pace --no-restore
pace --help
```

[config.example.toml](config.example.toml) documents the supported settings: three themes, nine accent colours, window zoom, font size and line height, scrollback limit, shell executable, cursor style/blink, sidebar width, workspace restoration, and close confirmation. Window zoom is available in Preferences under Appearance; changes there or through zoom shortcuts are saved and restored on the next launch, including with workspace restoration disabled. Settings are validated; unknown keys are rejected. Workspace restoration restores directories, split positions, and focused panes, and launches fresh shell processes; a remote workspace opens new SSH connections to its host. Commands and process memory are never serialized.

Pace also remembers the window's size and maximized state when closed. Window state is saved as `window.json` in the data directory, independently of workspace restoration; `--no-restore` and `restore_workspaces = false` only affect workspaces. Use `--size WIDTHxHEIGHT` to override the saved geometry and start with a non-maximized window. Screenshot launches use the default or explicit size and do not save window state.

## Keyboard shortcuts

Use Ctrl+Shift on Linux/Windows and Command on macOS: T opens a workspace, D splits right, E splits below, W closes the focused pane, F searches, P opens commands, B toggles the sidebar, Enter zooms the focused pane to full size and back, and 1–9 select a workspace by its sidebar position. Ctrl+Tab switches workspaces. Ctrl+Shift+Left/Right/Up/Down focuses the adjacent pane on every platform, including while zoomed; at an outer edge, focus stays put. These moves are also available in the command palette. Escape cancels a terminal drag, or leaves a sheet or a focused search field; otherwise it goes to the shell, as do Tab and unmodified arrow keys. Ctrl+comma opens preferences. Ctrl+plus/minus (Command on macOS) zooms the whole app; Ctrl+equals also zooms in, and Ctrl+0 resets app zoom (Command on macOS). On keyboards where Plus requires Shift, use Ctrl+equals (Command on macOS) for app zoom. Change terminal font size in Preferences or with Ctrl+Shift+plus/minus on Linux/Windows and Command+Shift+plus/minus on macOS; Ctrl+Shift+0 (Command+Shift+0 on macOS) resets it to the default (14 pt). Use Ctrl+Shift+C/V to copy/paste on Linux/Windows, Command+C/V on macOS. Plain Ctrl+C interrupts the shell; Shift+PageUp/PageDown scrolls history. Hold Shift to select text when a TUI owns the mouse.

The native [desktop icon](assets/pace.svg) and [Linux desktop entry](packaging/pace.desktop) are provided. Install the binary on your PATH and these files in your desktop environment's standard application/icon locations.

For a per-user Linux install (ensure `~/.local/bin` is on your PATH):

```sh
install -Dm755 target/release/pace ~/.local/bin/pace
install -Dm644 packaging/pace.desktop ~/.local/share/applications/dev.pace.terminal.desktop
install -Dm644 assets/pace.svg ~/.local/share/icons/hicolor/scalable/apps/pace.svg
```

## Verification

See [the verification record and release checklist](docs/verification.md) for test scope, native screenshots, and platform checks still required for distribution. Local development uses the narrowest meaningful affected crate/target tests; CI owns workspace-wide formatting, lint, tests and builds. Run repo-wide checks locally only when explicitly requested. The owning [agent guides](AGENTS.md) provide scoped commands.

Use the isolated native harness below for interactive review. For targeted framebuffer capture and Linux/X11 OS-input smoke testing, follow [the native verification procedure](docs/verification.md#native-application-verification), including a fresh data root and the exact owned window/inspection endpoint.

## Performance and Ghostty

PTY reading and terminal parsing happen outside the UI thread. History is bounded and the renderer consumes the visible viewport. Performance work focuses on lock duration, repaint coalescing, cached text layout, idle CPU, and sustained output. Benchmarks must use release builds. No claim of being faster than Ghostty or Alacritty is made without reproducible end-to-end measurements. See [measured performance and reproducible probes](docs/performance.md).

Ghostty's public `libghostty-vt` is a possible future engine, but it supplies terminal state and render-state extraction rather than GPU drawing. Its API is still unstable. The full Ghostty surface API is internal and tailored to its macOS application, so it is not the current cross-platform renderer. See [the architecture decision and release gates](docs/architecture.md) for primary sources, the migration plan, and the verification matrix.

## CI and artifacts

GitHub Actions runs CI on pull requests and merge queue groups. After change classification, fast formatting/architecture validation, the native platform matrix, and latest-stable compatibility run in parallel; documentation/evidence-only changes skip compilation within successful jobs. Merge queue classification includes all changes in the group relative to its base on `main`. Default and inspection configurations are tested on Linux, macOS, and Windows. Linux also runs Clippy and isolated X11 inspection/restoration using the same build, while a separate job checks all targets/features with latest stable Rust. CI uses an unoptimized profile without debug symbols and caches dependencies even after failed runs; performance measurements and releases use optimized builds.

CI uses read-only repository permissions and a timeout for every job. New commits cancel older CI runs for the same PR; merge queue runs remain independent. The final `Check` status succeeds only when every CI job succeeds, rejecting failed, cancelled, or skipped jobs. Once this workflow has reported `Check`, replace the individual CI status requirements in the `main` ruleset with `Check` from GitHub Actions. Existing status names remain available during that transition.

Changes to `main` require a pull request with passing required checks, then entry into GitHub's merge queue. The queue tests the combined changes against the latest `main` and any PRs ahead of them, then merges automatically when the required checks pass. PR branches do not need to be manually updated after every merge.

CI and release workflows use the standard GitHub-hosted `ubuntu-latest`, `macos-latest` (Apple Silicon), and `windows-latest` runners. Apple Silicon (`aarch64-apple-darwin`) is the primary macOS target for pull-request CI and release downloads. An additional Intel compatibility archive (`x86_64-apple-darwin`) is built and tested natively on `macos-15-intel` when release tags are pushed, using the same packaging steps.

Pushing a tag matching the package version, such as `v0.1.0`, runs release checks and packages Linux x64, macOS arm64/Intel, and Windows x64 binaries. The archives include configuration, licenses, dependency notices, and SHA-256 checksums, and are staged in a draft GitHub release for review.

These are binary archives, not installers. macOS signing/notarization, Windows signing, an update mechanism, and distribution-specific integration remain release work. Kitty graphics, full font fallback/shaping, keypad identification and some advanced keyboard modes, and platform accessibility need dedicated coverage before they are advertised. Running a GUI and reviewing screenshots on each OS remains necessary even after CI passes.

Pace is MIT licensed. Bundled fonts retain their SIL Open Font License notices; third-party dependencies and vendored development skills retain their respective licenses. See [third-party notices](THIRD-PARTY-NOTICES.md) for skill attribution and the full upstream license texts.

Developer inspection is opt-in through the `inspection` feature. `pace-inspect` exposes the native accessibility tree and supports `screenshot`, `key`, `text`, `click`, `context` (secondary click), `double-click`, `drag`, `press`, `release`, and `resize` commands for repeatable native visual review. The normal release binary has no inspection listener. Independent launches must follow [the run-isolation rules](scripts/AGENTS.md).

Native regression runs own their process, temporary data directory, and inspection
endpoint; they never use your saved workspaces:

```sh
cargo build -p pace-terminal --features inspection --locked --bin pace --bin pace-inspect
python3 scripts/native-harness.py --restore --output artifacts/native-review
```

Use `--diagnostics` for JSON counters with pane/session generations, resource
reservations, snapshot work, and frame p50/p95/p99. Terminal contents are excluded.
`python3 scripts/measure-scale.py --help` describes reproducible many-pane runs.
