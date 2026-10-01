# Desktop runtime

`sessions.rs` owns live, starting and closing sessions, startup workers and
aggregate resource policy. Reserve capacity before startup; retain reservations
until teardown completes, even after a pane leaves the model. Startup is bounded.
Restart replaces the same pane with a new generation and remains possible at pane capacity;
unrelated new panes cannot use replacement semantics to bypass limits. Dispose
of stale startup results and release their resources.

Shutdown requests remain nonblocking during frames. Account for pending starts,
deferred replacements and closing workers through intentional shutdown. Resource
budgets are logical reservations, not guarantees of process RSS.

`persistence.rs` owns one background writer for state and config. Frames publish
immutable snapshots; the writer coalesces each destination and debounces for
75 ms. Submitted generations are monotonic per destination. Acknowledgements
identify the exact destination/generation; an old save cannot clear newer dirty
state. Serialization, fsync and atomic replacement stay off the UI thread.
Intentional shutdown flushes the latest accepted snapshots with a timeout.
Ephemeral runs write neither destination. Workspace write protection does not
disable independent settings saves. File compatibility and recovery belong to
the sibling [persistence guide](../persistence/AGENTS.md).

Choose the affected behavior, not both suites by default:

```sh
cargo test -p pace-terminal --lib runtime::sessions::tests --locked
cargo test -p pace-terminal --lib runtime::persistence::tests --locked
```

Session fixtures use real Unix shells; `/bin/sh` is required. Writer tests use
isolated storage and injected slow/failing writes. When changing a lifecycle or
save seam, include the corresponding controller/application test for completion
handling. For measured many-pane behavior, use the applicable scenario in
[docs/performance.md](../../docs/performance.md) with the isolation rules in
[scripts/AGENTS.md](../../scripts/AGENTS.md).
