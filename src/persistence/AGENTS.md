# Workspace file compatibility

`workspace_state.rs` owns versioned DTOs independently of runtime layouts and
model internals. Preserve migration from `fixtures/workspaces-unversioned.json`.
Restore through the model's validation boundary; report skipped/repaired entries
with workspace/pane context rather than constructing invalid live state.

Unsupported future schemas and unreadable files block workspace writes. Damaged
or lossy state may be replaced only after a recovery copy of the original bytes
succeeds; failed recovery leaves it read-only. Missing directories need explicit
fallback/skip diagnostics. Workspace write protection must leave settings saves
available. Storage scheduling belongs to [runtime](../runtime/AGENTS.md), and
settings validation/atomic writes belong to `../config.rs`.

For schema/recovery changes, run the relevant case or this focused module:

```sh
cargo test -p pace-terminal --lib persistence::workspace_state::tests --locked
```

Include the writer/application completion path only when its behavior changes.
Use temporary fixture directories; do not migrate or replace the developer's
saved workspaces as test data.
