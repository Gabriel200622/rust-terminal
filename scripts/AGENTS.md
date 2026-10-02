# Native verification and probes

Prefer `native-harness.py` for integrated native verification: it owns a unique
loopback endpoint, fresh data directory, subprocess handles, logs, artifacts and
cleanup. Build only the required desktop targets:

```sh
cargo build -p neptune-terminal --features inspection --locked --bin neptune --bin neptune-inspect
python3 scripts/native-harness.py --output artifacts/native
```

For final release captures or performance probes, build those targets with
`--release` and pass their paths through `--app`/`--client`. Load the applicable
procedure in [docs/verification.md](../docs/verification.md) for native acceptance
or [docs/performance.md](../docs/performance.md) for CPU/RSS and scaling. Select
only relevant scenarios; use each script's `--help` for current arguments.

## Isolation hazards

- Independently launched test instances need a fresh `--data-root PATH`.
  Inspection-enabled launches also need a unique loopback
  `EGUI_INSPECTION=127.0.0.1:PORT`. Pass that exact endpoint and data root to
  inspection/regression scripts, including relaunches. An explicit `--config`
  must also refer to task-owned storage.
- Reap only captured subprocess handles/PIDs owned by this run. Regression close
  and restore operations may terminate/relaunch the app: verify the endpoint
  belongs to this task before sending them. Never attach mutating tests to the
  developer's working terminal or reuse its saved state.
- OS key/pointer workflows run sequentially because desktop focus is global.
  For `native-smoke.py`/`ui-regression.py`, pass the exact owned `--window-id`;
  do not let title matching select another Neptune window. Prefer stable accessible
  pane/field labels and bounded semantic polling in `inspect-regression.py` over
  coordinates or fixed delays. The old coordinate workflow is diagnostic only.

## Evidence and focused checks

The integrated shell regression needs a POSIX shell. X11 injection verifies
Linux/X11 input, not Wayland, macOS or Windows input. Inspection events verify
application routing; clipboard, IME and real OS keyboard behavior need native
checks. A passing interaction report still needs visual inspection of captures.

Output measurements require producer-start markers and parsed-byte evidence
before sampling, activity throughout the sample and complete session teardown.
Use release builds and distinguish parser throughput, frame CPU, process CPU/RSS,
inspection roundtrip latency and GPU/input-to-display performance. Record actual
host, features and build metadata; retain failed runs as diagnostic evidence.

For changes to the corresponding tool, choose one:

```sh
python3 -m unittest discover -s scripts -p test_native_harness.py
python3 -m unittest discover -s scripts -p test_measure_scale.py
python3 scripts/check-architecture.py --self-test
```

The architecture checker is a lightweight boundary proof for dependency/public
seam changes. Its self-tests are needed when changing the checker, not for every
Rust change. Do not launch native/resource scenarios for instruction-only edits.
