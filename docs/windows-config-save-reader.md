# Windows concurrent configuration-save reader

Investigation for [issue #22](https://github.com/zevem/neptune/issues/22),
2026-10-01.

## Observed failure and scope

The Windows `windows-2025` matrix failed twice at the stress test's
`std::fs::read(&path).unwrap()`, with error 5 (`ERROR_ACCESS_DENIED`): the
[original run](https://github.com/zevem/neptune/actions/runs/36825282335/job/110249579247)
and [failed-job rerun](https://github.com/zevem/neptune/actions/runs/36825282335/job/110251876724).
These are two failures in two recorded attempts, not a measured failure rate
from a repeated focused invocation. PR #13's writer-conflict handling is already
merged; it cannot retry a failed reader open.

`main` subsequently added a reader workaround in commit `4666306`: retry these
Windows conflicts with `yield_now()` while writers run. This change retains the
same error filter, adds a per-read deadline, and requires successful reader
progress plus explicit recovery/error/timeout regressions.

`tempfile` 3.27.0 replaces the Windows destination with
`MoveFileExW(MOVEFILE_REPLACE_EXISTING)`. Atomic contents do not establish that
every concurrent open succeeds. Microsoft's
[CreateFileW documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew)
identifies sharing conflicts and access denial when opening a delete-pending
file. A transient replacement conflict is consistent with the observed error;
the precise Windows filesystem mechanism has not been established by a native
trace or a new Windows reproduction.

The correction belongs to this stress-test reader: it deliberately reads while
eight independent writers continuously replace the destination. The application
loads configuration before starting its persistence writer, which serializes
saves. Production storage behavior and its existing writer deadline are unchanged.

## Regression coverage

Only the Windows stress-test reader retries raw Windows errors 5, 32 and 33,
with 5 ms pauses and a 250 ms deadline. Timeout, missing files and other I/O
errors still fail. Every successful observation must contain exactly 32 KiB of
one of the writers' repeated bytes. The test requires successful read and write
progress and no remaining temporary files; existing failed-save preservation
and cleanup assertions remain in place.

Injected-error tests exercise recovery, unexpected-error propagation and timeout
on every host. A Windows-only test opens a real file without sharing access,
waits for the reader to observe error 32, then releases the handle and requires
a complete successful read. It supplements the original replacement stress
test; it does not establish the mechanism behind the original error 5.

## Verification status

The local host is Linux with the pinned Rust 1.97.1 toolchain. Injected-error
regressions first failed with the single-read behavior. Focused Linux results
cannot establish Windows correctness; Windows evidence comes from PR CI.

With Rust 1.97.1, the `ci` profile and locked dependencies, the focused
`config::tests` filter passed all 10 applicable tests in both ordinary and
`inspection` configurations. The original concurrent-save test also passed
30/30 separate focused Linux invocations.

[CI run 36827946358](https://github.com/zevem/neptune/actions/runs/36827946358)
passed all six checks on PR #23's initial head `cf0bb6c`, including the complete
matrix and final native-inspection gate. The
[Windows job](https://github.com/zevem/neptune/actions/runs/36827946358/job/110257844454)
passed the original stress test, all injected-error regressions and the real
handle regression in both ordinary and inspection configurations on
`windows-2025` with Rust 1.97.1.

Those are two successful Windows CI stress invocations, not a measured repeated
standalone focused run. The precise mechanism behind the original error 5
remains unconfirmed. Updating the PR head to resolve the overlap with `main`
requires fresh CI; the initial run does not establish acceptance for that head.
