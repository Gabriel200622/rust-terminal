# Snapshot renderer

`geometry.rs` computes grid/pointer geometry; `cache.rs` prepares and caches owned
viewport data; `paint.rs` draws it and emits interactions. `src/terminal.rs` is a
compatibility export, not a second renderer. Painting receives no live session,
backend type or terminal lock. Session mutations and PTY resize effects return
to the desktop coordinator through targeted actions.

Preserve unchanged row/galley reuse and explicit invalidation for geometry,
display scale, fonts, palette and session replacement. Cursor, selection and
search damage should not reshape unchanged text. Search/selection coordinates
are terminal columns, including wide/combining cells, not UTF-8 offsets. Release
hidden cache allocations without stopping hidden session output.

The consumer must follow terminal-core's repaint, snapshot and resize ordering
contract in [its guide](../../crates/terminal-core/AGENTS.md). Keep shaping and
painting outside terminal locks; render only the viewport. Do not replace the
paint architecture without measurements of the actual bottleneck.

Choose synthetic/geometry proof first; use real PTY fixtures for transport-related
changes:

```sh
cargo test -p pace-terminal --lib terminal_view::geometry::tests --locked
cargo test -p pace-terminal --lib terminal_view::cache::synthetic_tests --locked
cargo test -p pace-terminal --lib terminal_view::cache::tests --locked
```

The real-PTY set requires Unix and `/bin/sh`. These tests do not submit GPU work.
For rendering changes, inspect affected native output, resize and DPI states via
[scripts/AGENTS.md](../../scripts/AGENTS.md); consult
[docs/performance.md](../../docs/performance.md) for measurement scope and
[docs/design.md](../../docs/design.md) for visual direction.
