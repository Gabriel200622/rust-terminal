# Desktop composition

`neptune-terminal` is the desktop package; `src/lib.rs` is its testable composition
surface, `main.rs` launches Neptune, and `bin/neptune-inspect.rs` is an optional
developer client. Do not move desktop services into the pure model or expose
terminal backend internals to satisfy a desktop caller.

## Find the owner

| Change | Entry points / local contract |
| --- | --- |
| Frames, bootstrap, search coordination | `app/mod.rs` |
| Actions, effects and close confirmation | `app/coordinator.rs` |
| Input ownership and egui normalization | `app/input.rs`, `input.rs` |
| Startup, restart, teardown and saving | [runtime/AGENTS.md](runtime/AGENTS.md) |
| Workspace schemas and recovery | [persistence/AGENTS.md](persistence/AGENTS.md) |
| Widgets, overlays and presentation data | [ui/AGENTS.md](ui/AGENTS.md) |
| Geometry, snapshot preparation and painting | [terminal_view/AGENTS.md](terminal_view/AGENTS.md) |
| Clipboard, font loading and window effects | `platform/` |
| Settings validation, palette and icons | `config.rs`, `theme.rs`, `icons.rs` |

The coordinator executes controller effects and feeds completions back through
the controller. Keep session handles, render caches, directory validation,
preferences, search and overlays desktop-owned.

Bootstrap loads settings/state on a worker. Preserve bounded pending actions,
coalesced preferences and the launch command's original pane/generation; focus
changes must not retarget or duplicate it. Close/restart/failure cancels delivery
to that session. A failed loader must preserve storage write protection.

Input routing gives overlays, editable fields, search and terminals explicit
ownership. Normalize egui events here; terminal-core encodes terminal protocols.
Preserve host shortcuts, ordinary shell Ctrl+C, IME/preedit and text/key ownership
without duplicate delivery. Close confirmation retains its original target even
if focus changes before confirmation.

Compile search on query changes and scan with row/time budgets. Match results to
query, terminal revision and session identity before using them. Keep clipboard,
font and window operations behind `platform/`; fake services establish logic,
not actual OS integration.

## Focused proof

Choose the relevant filter/target; these are alternatives, not a checklist:

```sh
cargo test -p neptune-terminal --lib app::tests --locked
cargo test -p neptune-terminal --lib input::tests --locked
cargo test -p neptune-terminal --lib config::tests --locked
cargo test -p neptune-terminal --lib platform:: --locked
cargo check -p neptune-terminal --lib --locked
cargo clippy -p neptune-terminal --lib --locked -- -D warnings
```

Narrow tests further by function name when appropriate. CLI changes can use
`cargo check -p neptune-terminal --bin neptune --locked`; inspection-client changes use
`cargo test -p neptune-terminal --bin neptune-inspect --features inspection --locked`.
Enable `inspection` only for affected feature paths. Format changed Rust files
with the pinned rustfmt. Dependency/seam changes can use the lightweight
`python3 scripts/check-architecture.py` boundary check.

For user-visible changes, use the native workflow in
[scripts/AGENTS.md](../scripts/AGENTS.md) and the relevant sections of
[docs/verification.md](../docs/verification.md). Apply the local UI/renderer
guides when changing `theme.rs`, `icons.rs` or fonts as well as widgets. Inspect
fresh captures of the affected states; reserve the full acceptance matrix for
release work or an explicit request.
