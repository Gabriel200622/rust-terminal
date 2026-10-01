# Windows concurrent configuration-save reader

Investigation for [issue #22](https://github.com/Gabriel200622/rust-terminal/issues/22),
2026-10-01.

## Observed failure and scope

The Windows `windows-2025` matrix failed twice at the stress test's
`std::fs::read(&path).unwrap()`, with error 5 (`ERROR_ACCESS_DENIED`): the
[original run](https://github.com/Gabriel200622/rust-terminal/actions/runs/36825282335/job/110249579247)
and [failed-job rerun](https://github.com/Gabriel200622/rust-terminal/actions/runs/36825282335/job/110251876724).
These are two failures in two recorded attempts, not a measured failure rate
from a repeated focused invocation. PR #13's writer-conflict handling is already
merged; it cannot retry a failed reader open.

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

The available host is Linux with the pinned Rust 1.97.1 toolchain. Injected-error
regressions first failed with the single-read behavior. Focused Linux results
cannot establish Windows correctness. No actual Windows machine is available
for this task, and CI currently runs only on pull requests.

With Rust 1.97.1, the `ci` profile and locked dependencies, the focused
`config::tests` filter passed all 10 applicable tests in both ordinary and
`inspection` configurations. The original concurrent-save test also passed
30/30 separate focused Linux invocations. Windows-only tests were not run.

Windows acceptance remains pending: repeat the exact focused invocation from
issue #22, record attempts/failures, run the Windows-only handle regression,
and require ordinary and inspection CI configurations plus the final native
inspection gate to pass. This note does not claim those gates have passed.
