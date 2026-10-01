# Pure model ownership

This crate owns ordered workspaces, pane membership, stable identities, layout
invariants and generation-tagged lifecycle. It may depend on `serde`; it must not
depend on egui/eframe, terminal-core, Alacritty, a PTY, filesystem or workers.

`WorkspaceSpec` is construction data, not an unchecked mutable workspace.
`Model::restore` validates globally unique nonzero identities, exactly one layout
leaf per pane, split ratios, active membership and limits before accepting it.
Live fields remain private. Preserve unrelated split identities on tree edits.

All durable mutations go through `Controller::dispatch`. Commands capture their
targets when queued. Effects describe work; they do not perform it. Changing
workspace/pane focus produces the same focus/search effects from every entry
point. Successful saves acknowledge their exact generation; an old completion
cannot clear newer dirty state. Restart replaces a session and remains possible
at pane capacity. Stale start/fail/exit/cwd completions are ignored.

The desktop owns directory validation, close-confirmation overlays, preferences,
search, render caches and session handles. Do not introduce those responsibilities
here. The fake runtime tests exercise real commands and effects without shells.

For a controller/layout change, run the affected test or module filter; the
crate suite is small and headless when the whole model contract changes:

```sh
cargo test -p pace-model --locked
cargo clippy -p pace-model --lib --locked -- -D warnings
```

Use `python3 scripts/check-architecture.py` for dependency/public-boundary
changes. The fake runtime is the normal proof here; do not start shells or the
desktop to verify pure transitions.
