# Pace

## Never compromise on

- **A real, native terminal.** Pace is a Rust desktop application with independent
  PTY shell sessions and GPU rendering, without a browser/webview. Terminal
  content takes priority over chrome; the default view is a working shell.
- **Responsiveness under load.** Keep startup, I/O, parsing and storage work out
  of interactive frames. Bound queues, history, search and aggregate resources;
  hidden panes keep processing output. Quiet terminals sleep between events and
  cursor deadlines.
- **Correct state and recoverable work.** Preserve terminal input, output and
  history through resize and lifecycle changes. Workspace restoration retains
  organization and directories and starts fresh shells; it does not restore live
  processes or serialize commands/process memory. Protect damaged or unsupported
  saved state before allowing replacement.
- **Honest evidence and private contents.** Diagnostics exclude terminal contents
  and input. Platform support and performance claims require evidence from the
  actual host/build; dependencies, configured CI jobs and parser benchmarks alone
  do not establish native correctness or PTY-to-screen speed.

## Architecture and change impact

Keep `desktop → pace-model` and `desktop → terminal-core`. The pure model has no
GUI, filesystem, worker, PTY or terminal-engine dependency. Runtime owns processes;
rendering consumes owned viewport snapshots, without live sessions or locks.
Terminal backend types stay private to terminal-core.

Durable model mutations go through `Controller::dispatch`. Capture stable pane,
workspace and split targets when actions are created. Session completions carry
pane generation; stale results cannot update a closed or replacement pane.

For behavior changes, trace every applicable entry point: shortcuts, sidebar,
pane controls, palette, dialogs, CLI and inspection. Check affected public APIs,
saved formats, settings and supported OS paths together. Include starting,
running, hidden, exited, failed and closing states; event/repaint notifications,
backpressure, retries, cancellation and reverse actions such as restart or close
cancel. Update affected user guidance and links when their meaning changes.

User-visible UI changes require visual review of the running native application
after the final build, including affected states and narrow windows. Headless
tests and successful interaction reports do not establish visual acceptance.
In UI reports, embed a running-app screenshot of **every changed screen** from
the native desktop app, mobile emulator or web browser, as applicable. If a
screenshot is unavailable, name the screen and explain why.

## Engineering taste

Understand the real constraint, then prefer the smallest correct implementation.
Follow established ownership, types and patterns before adding abstractions,
machinery or dependencies. Keep domain rules explicit and testable at their
owning boundary. Avoid speculative generality and unrelated refactors; measure
before changing the renderer or worker architecture for performance.

## Authority and contextual reading

Approved product, architectural, legal/license, privacy and payment decisions
outrank existing implementation. Surface conflicts; never rewrite an
authoritative decision merely to legitimize current code. Update durable
preferences or agent instructions only when explicitly requested; do not infer
new standing preferences from a task or retrospective.

Read only the sources relevant to the change:

| When changing | Guidance / source of truth |
| --- | --- |
| Ownership, dependencies, engine or renderer choice | [docs/architecture.md](docs/architecture.md) |
| Workspace model and controller | [crates/pace-model/AGENTS.md](crates/pace-model/AGENTS.md) |
| Terminal semantics and transport | [crates/terminal-core/AGENTS.md](crates/terminal-core/AGENTS.md) |
| Desktop composition, input, platform or UI | [src/AGENTS.md](src/AGENTS.md) and the touched directory's guide |
| Native automation or resource measurements | [scripts/AGENTS.md](scripts/AGENTS.md) |
| Release/support claims | Remaining gates in [docs/verification.md](docs/verification.md) |

Review reports and dated acceptance records are historical evidence, not new
product decisions. Consult accepted ADRs for the area if present. Use existing
focused docs and `.agents/skills/` for procedures when the task calls for them;
these repository constraints also apply when following a skill.

## Safe development and verification

- Changes arrive by PR from a feature branch into `main`, including hotfixes.
  Never commit directly to `main`, or directly push or force-push to a protected
  branch. CI must be green before merge. For branch, PR and merge operations,
  read [the issue tracker guide](docs/agents/issue-tracker.md).
- Preserve the branch/worktree in which the task was launched unless the task
  requires changing it. Start new issue/feature branches from `main`; do not
  branch from unrelated feature work. Never overwrite, reset, clean or stage
  unrelated work.
- Track processes/services started by this task and stop only those. Never kill
  by application name, path or worktree pattern, or stop another task's services.
  Native test launches use fresh `--data-root` storage; `--config` and
  `--no-restore` do not isolate saved workspace writes. Follow the script guide
  before launching or driving Pace; desktop input focus is shared across tasks.
- **Do not run repo-wide checks locally unless explicitly requested. CI owns
  the full suite.** Run the narrowest meaningful proof: focused tests, affected
  crate/target checks and targeted integration tests where behavior requires
  them. Cross-boundary work needs proof at each changed seam, not an automatic
  workspace build/test/lint sweep. Documentation-only changes need reference and
  consistency review, not Rust builds.
- Use the pinned `rust-toolchain.toml` and `--locked`; do not upgrade the
  toolchain or lockfile merely to get a check running. Local commands belong in
  the owning guide. Report actual checks, host/features and any unverified
  native/platform behavior.

## Agent skills

### Issue tracker

Issues and specs live in GitHub Issues for `Gabriel200622/rust-terminal`. For ticket operations, read [the issue tracker guide](docs/agents/issue-tracker.md).

### Triage labels

Use `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix`. For triage, read [the label mapping](docs/agents/triage-labels.md).

### Domain docs

Use a single root `GLOSSARY.md` and `docs/adr/`. For domain vocabulary and decisions, read [the domain guide](docs/agents/domain.md).
