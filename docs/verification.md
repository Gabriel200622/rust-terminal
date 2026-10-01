# Verification and release readiness

Pace has a running native Linux implementation with real PTY sessions, automated
engine and application tests, and iterative native screenshot review. Linux is
the local verification platform. macOS and Windows build/test jobs are configured
in CI; their existence does not establish a successful CI run or desktop runtime
verification. A production release requires the remaining gates below.

## Local proof and CI coverage

Local changes use the narrowest meaningful affected crate/target checks and
focused behavior tests. See [desktop guidance](../src/AGENTS.md),
[model guidance](../crates/pace-model/AGENTS.md),
[terminal guidance](../crates/terminal-core/AGENTS.md) and
[script guidance](../scripts/AGENTS.md) for scoped commands. Cross-boundary work
checks each changed seam. Documentation-only edits need reference/consistency
review rather than Rust builds.

**Do not run repo-wide checks locally unless explicitly requested. CI owns the
full suite.** The commands below document full-suite reproduction for such a
request; they are not a local handoff checklist. The actual CI matrix and commands
are maintained in [ci.yml](../.github/workflows/ci.yml).

Unix PTY tests require `/bin/sh` and `zsh`. Install `zsh` on Linux if it is
absent; the multiline prompt regression uses an isolated shell configuration.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 scripts/check-architecture.py
python3 scripts/check-architecture.py --self-test
cargo build --release --locked --bin pace
```

Rust 1.97.1 is pinned by `rust-toolchain.toml` and is the declared minimum for all
packages. CI tests default and inspection configurations on that baseline on
Linux, macOS and Windows. Latest stable gets an all-target/all-feature compile
check rather than a second copy of the Linux test suite. Formatting and
architecture checks precede compilation; documentation/evidence-only PRs skip
the build matrix while retaining successful required statuses. The `ci` Cargo
profile disables optimization/debug symbols, and dependency caches are saved on
failure. Linux native inspection/restoration reuses the matrix job's compiled
dependencies instead of starting a separate cold GUI build. These checks are
correctness evidence; performance measurements still require release builds.
Current coverage includes:

| Boundary | What is exercised |
| --- | --- |
| Real Unix PTY | Input, parser replies, resize, exit status, idle shutdown, escalation of a shell ignoring SIGHUP, input/grid limits, bracketed paste, synchronized-output timeout, and real-zsh multiline prompt/input preservation |
| Resize and backpressure | Native resize waits for terminal-grid access; a rejected resize preserves existing grid dimensions when the input queue is full |
| Terminal input | Ctrl combinations, application cursor mode, modified/function keys, Unicode text ownership, Kitty key flags/repeat/release, SGR/legacy mouse coordinates, and motion modes |
| Renderer cache | A real quiet shell and headless egui frames retain galley identity when cells are unchanged; one changed row rebuilds once; wide/combining Unicode search highlights use terminal columns; a display-scale change rebuilds glyph layouts at unchanged grid dimensions and subsequent frames reuse them |
| Custom controls | A right-aligned titlebar icon retains its widget identity across hover/press/release and clicks once; hiding a neighboring control preserves the focused action, so Enter cannot activate a different button; switches, segmented choices, sliders and steppers change once per interaction and respect their bounds; a new dialog gives its first field the keyboard once visible; command-palette entries capture their pane and filter by query; dragging a sidebar row moves its workspace once on release, scrolls a long list from its edge, leaves a click selecting, and is cancelled by Escape without passing the key on |
| Configuration and layout | Resource validation, serialization, atomic replacement, failed/concurrent writes, saved split validation, split removal, and Unicode-safe labels |
| Pure application controller | Targeted commands, workspace order/identity and reordering, closing before/at/after focus, stable split identities, pane moves within and between workspaces (rearrange, swap, unchanged drops, roomiest-pane placement, last-pane removal, capacity and atomic rejection), failed/stale startup, restart at capacity, fake-runtime effects and dirty/save acknowledgements |
| Persistence compatibility and scheduling | Unversioned fixture migration, independent versioned DTOs, invalid/missing directory recovery, corrupt-file preservation, unsupported/unreadable write protection, coalescing, destination generations, slow/failing storage, bounded flush and ephemeral isolation |
| Architecture | Forbidden model dependencies (including target tables/aliases), desktop backend/lock leaks and production renderer live-session leaks; checker self-tests verify rejection |
| Asynchronous bootstrap | Bounded pending actions replay onto restored state; preference changes coalesce; loader failure preserves storage; launch commands retain their original pane/generation and are delivered once to a real PTY despite focus changes |

Renderer tests include synthetic project-owned snapshots across platforms and a
smaller real-PTY integration set. They do not submit GPU work. Unix PTY fixtures do not run on Windows. Dedicated ConPTY
fixtures are provided for Windows and require execution there; they are not
covered by a Linux test result.

### Shell prompt resize coverage

The engine observes OSC 133 semantic primary-prompt markers alongside the VT
parser. On resize it clears an identified redrawable active prompt before the
shell redraws it, preserving command output and history. Output markers end that
active region; continuation/right-prompt markers, `redraw=0`, and alternate-screen
content retain their separate semantics.

The automated real-zsh fixture uses an isolated `.zshrc` with OSC 133 hooks and
a synthetic multiline/right-aligned prompt. It fills the 10,000-row history cap,
types a long unsubmitted command, resizes between 100 and 33 columns and between
eight and four rows, then verifies prompt count, the typed buffer, and executed
command/history preservation. This targets actual ZLE redraw behavior without
depending on a developer's theme installation.

Local interactive screenshots additionally exercise the development host's real
zsh configuration. Its `.zshrc` sources `~/powerlevel10k/powerlevel10k.zsh-theme`
and `~/.p10k.zsh`, and contains existing BridgeSpace `precmd`/`preexec` hooks that
emit OSC 133 A/C markers for interactive shells. The final native workflow
preserved an actual 330-byte typed payload across 900×640 and 640×480 windows,
then verified its exact successful execution without clearing the terminal.

That observation verifies this sourced configuration; it does not mean Pace
installs shell-integration hooks. OSC 133 coverage does not establish universal
resize behavior for every shell, theme, asynchronous-output pattern, or
shell-integration implementation.

## Native application verification

Build the release executable before measuring or accepting final screenshots.
Capture the application's native framebuffer with a one-shot launch:

```sh
target/release/pace --data-root /tmp/pace-capture-UNIQUE --no-restore --screenshot artifacts/native-main.png
```

Replace `/tmp/pace-capture-UNIQUE` with a fresh task-owned directory. This mode
captures after three seconds and exits without saving workspace state.
It is useful for the default view; interactive states need a running application.
The preferred repeatable UI workflow is
[`native-harness.py`](../scripts/native-harness.py), which owns a unique endpoint,
fresh `--data-root`, process, logs, artifacts and cleanup. It invokes the semantic
[`inspect-regression.py`](../scripts/inspect-regression.py) workflow and carries
the same endpoint/data directory through restoration. Widgets expose stable
accessible pane and field labels. Build the optional developer tools and run:

```sh
cargo build -p pace-terminal --locked --features inspection --bin pace --bin pace-inspect
python3 scripts/native-harness.py --output artifacts/native
```

Each run writes `run.json`, `app.log`, `ui-regression.json` and captures beneath
its unique output directory. Native readiness and semantic conditions use bounded
polling. The full shell workflow currently requires a POSIX shell; OS key/pointer
injection remains in the explicitly Linux/X11 adapter.

For an independently managed manual launch, supply a fresh `--data-root` and
unique loopback `EGUI_INSPECTION=HOST:PORT` to Pace, then carry both to the script.
Any explicit `--config` must use task-owned storage too. Track the process at
spawn and stop only that process; do not find processes to kill by name/path.
`--config` or `--no-restore` alone does not isolate workspace writes:

```sh
python3 scripts/inspect-regression.py --addr HOST:PORT --data-root PATH \
  --output artifacts/native-regression --restore
```

The harness requires Python 3 and the inspection binaries. It checks native
field values, persisted workspace/theme state, independent interactive terminal
bodies, shell command markers, delivery of a literal Tab to the foreground
process with the terminal keeping keyboard focus, the terminal context menu and
its dismissal, sidebar resizing and reset from its edge, the Preferences content
viewport, search, and close/cancel behavior. It captures right/below splits, the terminal menu, Graphite/Light
preferences, and full-size/narrow windows. The harness verifies restoration by default and
cleans up its owned process. `--no-restore-check` skips that part. A manually
managed regression without `--restore` leaves its application running after
cancelling close.

Accept a run only when its `ui-regression.json` has `status: "passed"` and each
recorded check passed. A failed or interrupted run remains diagnostic evidence.
Record the exact executable/build, window size, desktop scale, and host alongside
the screenshots. Developer inspection proves application event routing and
native rendering; use the normal release build for performance measurements.

### Operating-system input smoke tests

[`native-smoke.py`](../scripts/native-smoke.py) complements inspection by sending
real X11 keyboard/pointer events and capturing the desktop window. It requires
Python 3, Pillow, libX11, and libXtst, and never starts the application. Launch
your isolated Pace instance through X11/XWayland (unset `WAYLAND_DISPLAY` only
for that child), identify its exact window ID, and pass it to every invocation:

```sh
python3 scripts/native-smoke.py --window-id 0xWINDOW info
python3 scripts/native-smoke.py --window-id 0xWINDOW key ctrl+shift+t
python3 scripts/native-smoke.py --window-id 0xWINDOW snapshot artifacts/native-keyboard.png
```

Replace `0xWINDOW` with the owned window ID, not a title match. Run OS-input
workflows sequentially; desktop focus is global across tasks.

The older coordinate-based `ui-regression.py` is retained for diagnostics;
its modal offsets require review after layout changes. Prefer the semantic
inspection workflow for repeatable UI regression. X11 automation does not
verify the Wayland input path.

### Normal-release runtime evidence

The normal Linux release was also exercised with native X11/XTest input at 2×
display scale. The run opened `nvim --clean -R README.md`, paged with Ctrl+F,
moved with Down, resized from 1180×760 to 900×640 logical points, and quit with
`:q`. The alternate screen returned to the existing shell, and a subsequent
`printf` command produced a file whose exact sentinel content was checked.
The [full-size TUI](../artifacts/native-nvim.png),
[resized TUI](../artifacts/native-nvim-resized.png), and
[returned shell](../artifacts/native-tui-return.png) captures show aligned grid
content, preserved native status lines, and clean font/icon rendering.

The [final normal-release workspace](../artifacts/native-final.png) combines a
read-only Neovim session, a ready shell after `ls src`, and actual parser-benchmark
output. Its three-pane hierarchy, consistent header spacing, and restrained
lavender marker clearly identify the focused top-right shell without competing
with terminal content. This capture passed the final designer review.

A separate normal-release Wayland run at 1.5× scale used the Vulkan/NVIDIA
renderer and displayed real shell output from ten passing terminal-core tests.
Its [native capture](../artifacts/native-wayland.png) verifies launch, terminal
output, and rendering at that scale. This smoke test does not establish complete
Wayland keyboard, pointer, clipboard, or IME coverage; the broader platform and
desktop-integration gates below remain open.

### Optional native inspection

The developer client also supports ad hoc native screenshots, logical-coordinate
input, Unicode text injection, and AccessKit tree inspection. With the application
running as above, use another terminal:

```sh
target/debug/pace-inspect --addr HOST:PORT info
target/debug/pace-inspect --addr HOST:PORT tree
target/debug/pace-inspect --addr HOST:PORT settle
target/debug/pace-inspect --addr HOST:PORT screenshot artifacts/inspection/main.png
```

Inspection screenshots use one pixel per logical point. X11 window captures use
native desktop pixels, so their dimensions differ at high display scale. The
normal release binary does not include the inspection listener. Injected events
verify application routing; they do not replace operating-system keyboard,
clipboard, IME, or screen-reader checks.

## Architecture refactor verification: 2026-09-30

On Linux with Rust 1.97.1, the completed refactor passes **125 default-feature
Rust tests** and **127 inspection-feature Rust tests**. Formatting and both
Clippy configurations pass with warnings denied. Architecture checking and its
three rejection self-tests pass, along with the Python harness/metric tests.
The Rust totals comprise 14 pure-model tests, 61 desktop tests, 37 terminal-core
tests, 13 Unix PTY tests and, with inspection enabled, two client tests.
The [default test log](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/test-default.log)
and [inspection test log](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/test-inspection.log)
retain the final results.

The final optimized inspection build passed **18 native semantic checks** and
produced **23 screenshots** in the
[accepted native run](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/ui-regression.json).
The [run metadata](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/run.json)
records compiler, platform, binary/lockfile hashes, endpoint and isolated state.
The workflow exercised independent shell processes, Ctrl+C, theme/preferences,
search ownership, close cancellation, graceful save/relaunch restoration and
exact preservation/execution of a 330-byte typed payload through two resizes.
Fresh three-pane and narrow-window captures were visually inspected. Events
came through native egui inspection; this is separate from OS keyboard/IME
coverage.

The ordinary release build also rendered a native screenshot, executed an exact
launch-command marker, left ephemeral workspace storage untouched and ignored
`EGUI_INSPECTION` (no listener), using both a controlled `/bin/sh` configuration
and the host's default zsh. See the
[ordinary-build record](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/normal-release.json)
and [zsh record](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/normal-zsh-release.json).
An initial host-zsh attempt contained an extra character before the command;
that capture is retained as `normal-zsh-first-attempt.png`. The same invocation,
three additional native launches and a direct core-session probe subsequently
passed without code changes. Its origin was not established; deterministic
automated launch-command coverage uses isolated controlled shells.

The optimized scaling build passed **19 applicable scenarios** across
1/8/32/64 panes, with the one-pane hidden-output case marked inapplicable.
Every accepted case confirmed complete session teardown. Output cases include
startup markers, parsed-byte verification and an activity audit; the 64-pane
several-output case was rerun after its first producers ended too soon.
The [accepted scenario summary](../artifacts/architecture-scale-accepted/accepted-summary.json)
links the original and replacement records. See [performance.md](performance.md)
for workload and measurement limits. These runs preceded the final failure-only
diagnostic additions; their recorded executable hashes identify the measured
build independently of the final ordinary executable.

After the final diagnostic/error-handling changes, the rebuilt ordinary release
passed [two targeted native smokes](../artifacts/architecture-accepted/20261001T012656Z-6ff804df/final-smokes/report.json):
successful exact command execution and an unavailable-shell failure. The latter
verified a visible error, a structured `Spawn` category with pane/generation,
complete teardown and no execution of the pending launch command. Both confirmed
inspection was disabled and ephemeral storage remained unchanged.

## Interface rebuild verification: 2026-09-30

The interface was rebuilt on this date; [design.md](design.md) records the
direction and the corrections made during review. Evidence is under
[`artifacts/ui-rebuild`](../artifacts/ui-rebuild).

- **Focused tests.** The desktop library suite passes 85 tests on Linux with
  Rust 1.97.1 (`cargo test -p pace-terminal --lib --locked`), and the
  inspection client's two tests pass. Clippy with warnings denied passes for the
  desktop package's targets with and without `inspection`; rustfmt and the
  architecture boundary check pass. The workspace-wide suite, `pace-model` and
  `terminal-core` tests were not rerun locally; neither crate changed.
- **Native regression.** The optimized inspection build passed 24 semantic
  checks and produced 25 captures in the
  [accepted run](../artifacts/ui-rebuild/20261001T034328Z-2d49bbe2/ui-regression.json);
  its [metadata](../artifacts/ui-rebuild/20261001T034328Z-2d49bbe2/run.json)
  records the binary and lockfile hashes. The run used X11 through the harness.
  New checks cover Tab delivery with the terminal keeping focus, the terminal
  menu and its dismissal, and sidebar resize and reset. An
  [earlier run](../artifacts/ui-rebuild/20261001T033954Z-4724214b/ui-regression.json)
  failed at the sidebar reset: the handle accepted only an exact double-click,
  and the toolkit counted the clicks as a triple because another click had just
  happened. Both resize handles now accept either. That run is retained as
  diagnostic evidence.
- **Captures.** The run's captures and separate manual captures of states the
  harness does not reach were inspected. Capture 17 was taken while two
  restored shells were still drawing their startup prompt; a resize trace of a
  separate restoration showed both resizes of every pane within 34 ms of launch
  and none later, and capture 18 shows the settled prompts.
- **Native Wayland.** The ordinary release build rendered
  [a one-shot capture](../artifacts/ui-rebuild/wayland-release.png) at 1.5×
  scale with transparent window corners, and was observed idle; see
  [performance.md](performance.md#native-wayland-idle-after-the-interface-rebuild).
  Wayland keyboard, pointer, clipboard and IME behaviour were not exercised.

Not verified: macOS and Windows builds and appearance (the Windows target is
not installed locally; CI owns those jobs), operating-system keyboard injection,
clipboard, screen-reader navigation of the new controls, and an input method
that is actively composing.

## SSH workspace verification: 2026-10-01

Remote workspaces were exercised on the Linux development host (GNOME Wayland
session, NVIDIA/Vulkan, OpenSSH 10.2p1) with a debug build that had the
`inspection` feature enabled.

- **Headless contracts.** Model tests cover destination validation, inherited
  destinations for new, split, restarted and restored panes, replacement of
  every pane when a workspace connects or disconnects, and atomic refusal.
  Persistence tests cover the version 2 round trip, lossless reading of
  version 1 and skipping an unusable saved destination. Desktop tests cover the
  client's argument vector, the sheet, palette and confirmation flow, the
  deferred startup path, the startup-command guard, and a real PTY spawn of a
  stand-in client proving that a directory reported by the host never replaces
  a pane's local directory.
- **Real client and server.** A task-owned OpenSSH server listened on loopback
  with throwaway keys; the review launch put a pass-through `ssh` first on
  `PATH` that only added `-F` with a task-owned client configuration. Through
  the X11 inspection path, connecting a two-pane workspace from its menu
  produced two accepted public-key logins, a split a third, and "Reconnect"
  after `exit` another; `SSH_CONNECTION` was set in the panes. Relaunching
  restored the three-pane layout with three new logins. Disconnecting returned
  all three panes to local shells in their original directory and removed the
  destination from saved state.
- **States reviewed from fresh captures.** Both workspace menus, the connect
  and new-workspace sheets (empty, unusable host, Graphite and Light), the
  palette commands, connected single/split panes, an ended connection, the
  disconnect confirmation, a missing client, and 900×640 and 640×480 windows.
  The review found that Enter on an unusable host left the field without the
  keyboard; the sheet now returns focus to it.
- **Wayland.** A one-shot `--ssh` capture at 1.5× scale rendered the connected
  workspace, and an idle `--diagnostics` sample settled at one to two frames a
  second for both a remote and a local workspace.

Not established: a host on another machine or a slow or lossy network, password,
passphrase and host-key prompts, agent forwarding and `ControlMaster` sharing,
and any behaviour on macOS or Windows, where the system client and ConPTY path
are unexercised. The standard isolated regression harness passed with the
version 2 state file.

## Historical visual acceptance

The following acceptance record predates the architecture refactor. It is useful
historical evidence, not acceptance of the current build. Fresh harness artifacts
are required after the changes described in the architecture review appendix.

On 2026-09-30, the final native inspection workflow passed **18 semantic checks**
and produced **23 fresh screenshots**. Its
[machine-readable report](../artifacts/final-review/ui-regression.json) records
workspace creation, three distinct shell processes, interruption, preferences,
responsive focus/search, close/cancel, restoration, and typed-input preservation.
The final designer review accepted the single/three-pane hierarchy, both theme
forms, contained narrow scrolling, vector search controls, focus markers, and
clean restored state. Earlier failed captures remain diagnostic evidence rather
than acceptance results.

For ordinary UI changes, review fresh native captures of affected states after
the final build, including narrow-window behavior. The full release acceptance
matrix (or an explicitly requested full design review) covers:

- A quiet single pane and a populated three-pane workspace.
- Workspace creation, preferences in Graphite and Light, search, and close
  confirmation with cancellation.
- 1180×760, 900×640, and 640×480 logical windows at the recorded display scale.

Inspect terminal dominance, focused-pane clarity, icon alignment, text contrast,
path truncation, overlay padding, field/action alignment, and narrow-window
clipping. Check typography, whitespace, color balance, density, and visual
character as one system. Resolve defects, rebuild, and recapture affected states
before accepting them. The design choices and corrections are recorded in
[design.md](design.md).

## Remaining production release gates

| Gate | Required evidence |
| --- | --- |
| Native platforms | Successful release builds and interactive desktop runs on Linux Wayland/X11, Windows ConPTY, and macOS; real shell/TUI input, resize/reflow, alternate screen, mouse, clipboard, and lifecycle verification on each |
| Text and desktop integration | Mixed DPI/display changes, non-US layouts and AltGr, IME/composition, installed/missing CJK and symbol fallbacks, complex scripts, accessibility navigation with a screen reader, and host window controls |
| Sustained behavior | Long output runs, bounded maximum-history memory, many independent panes, hidden-session output, rapid split/resize/close, exited processes, and visible error recovery |
| Performance | Repeated release measurements of PTY-to-screen throughput, keyboard-to-display latency, frame-time percentiles, startup, split-pane scaling, focused/unfocused idle with both blink settings, CPU/RSS, and GPU memory |
| Distribution | Test downloaded archives on clean machines; verify dependency/font notices and checksums, desktop integration, macOS signing/notarization, Windows signing, installer/update decisions, and versioned rollback behavior |

The parser benchmark and Linux CPU/RSS probe are documented in
[performance.md](performance.md). They do not measure GPU presentation or prove
comparative speed. Publish explicit workload, build, hardware, sample conditions,
and distributions for performance claims.

Current scope excludes Kitty graphics and comprehensive complex-script shaping.
Keyboard encoding also inherits egui's missing keypad identity, lock-state, and
some physical/layout information. Workspace restoration launches new shells, or
new SSH connections for a remote workspace; it does not restore live processes. Those boundaries must remain explicit in release
notes until implementation and native verification expand them.
