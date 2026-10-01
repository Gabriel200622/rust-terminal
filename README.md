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

When no saved workspace can be restored, Pace opens a terminal in your home directory (`~`). Use `--cwd` to choose another starting directory.

New workspace opens and selects a fresh shell at `~` immediately. Rename it later by double-clicking its sidebar row or choosing Rename workspace from its menu or the command palette.

```sh
pace --cwd /path/to/project
pace --config config.example.toml
pace --data-root /path/to/isolated-pace-data
pace --command 'printf "hello\n"'
pace --no-restore
pace --help
```

[config.example.toml](config.example.toml) documents the supported settings: three themes, nine accent colours, font size and line height, scrollback limit, shell executable, cursor style/blink, sidebar width, workspace restoration, and close confirmation. Settings are validated; unknown keys are rejected. Workspace restoration restores directories, split positions, and focused panes, and launches fresh shell processes. Commands and process memory are never serialized.

## Keyboard shortcuts

Use Ctrl+Shift on Linux/Windows and Command on macOS: T opens a workspace, D splits right, E splits below, W closes the focused pane, F searches, P opens commands, B toggles the sidebar, Enter zooms the focused pane to full size and back, and 1–9 select a workspace by its sidebar position. Ctrl+Tab switches workspaces. Escape cancels a terminal drag, or leaves a sheet or a focused search field; otherwise it goes to the shell, as do Tab and the arrow keys. Ctrl+comma opens preferences. Ctrl+plus/minus (Command on macOS) zooms the whole app; Ctrl+equals also zooms in, and Ctrl+0 resets app zoom (Command on macOS). On keyboards where Plus requires Shift, use Ctrl+equals for app zoom. Change terminal font size in Preferences or with Ctrl+Shift+plus/minus; Ctrl+Shift+0 resets it to the default (14 pt), on every platform. Use Ctrl+Shift+C/V to copy/paste on Linux/Windows, Command+C/V on macOS. Plain Ctrl+C interrupts the shell; Shift+PageUp/PageDown scrolls history. Hold Shift to select text when a TUI owns the mouse.

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

GitHub Actions runs CI only on pull requests. Fast formatting and architecture checks run before builds; documentation/evidence-only changes skip compilation. Default and inspection configurations are tested on Linux, macOS, and Windows. Linux also runs Clippy and isolated X11 inspection/restoration using the same build, while a separate job checks all targets/features with latest stable Rust. CI uses an unoptimized profile without debug symbols and caches dependencies even after failed runs; performance measurements and releases use optimized builds.

Pushing a tag matching the package version, such as `v0.1.0`, runs release checks and packages Linux x64, macOS arm64/Intel, and Windows x64 binaries. The archives include configuration, licenses, dependency notices, and SHA-256 checksums, and are staged in a draft GitHub release for review.

These are binary archives, not installers. macOS signing/notarization, Windows signing, an update mechanism, and distribution-specific integration remain release work. Kitty graphics, full font fallback/shaping, keypad identification and some advanced keyboard modes, and platform accessibility need dedicated coverage before they are advertised. Running a GUI and reviewing screenshots on each OS remains necessary even after CI passes.

MIT licensed. Bundled fonts retain their SIL Open Font License notices; third-party dependencies retain their respective licenses.

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
