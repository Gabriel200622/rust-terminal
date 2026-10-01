#!/usr/bin/env python3
"""Launch and verify one isolated native inspection build of Pace.

Each run owns a loopback endpoint, data directory, logs, screenshots and the
process it starts. Build first with `cargo build --features inspection --locked`.
The full regression currently requires a POSIX shell. Actual OS key injection
is covered by the separate Linux/X11 native-smoke.py adapter.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import hashlib
import os
from pathlib import Path
import platform
import socket
import subprocess
import sys
import time
import uuid


REPO = Path(__file__).resolve().parents[1]
SUFFIX = ".exe" if os.name == "nt" else ""


def free_endpoint() -> str:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reservation:
        reservation.bind(("127.0.0.1", 0))
        return f"127.0.0.1:{reservation.getsockname()[1]}"


def stop_owned(process: subprocess.Popen, timeout:float=5) -> None:
    """Reap only a Popen handle created by this run; never search by app name."""
    if process.poll() is not None:
        process.wait()
        return
    process.terminate()
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def inspect(client: Path, address: str, *arguments: str) -> dict:
    result = subprocess.run(
        [str(client), "--addr", address, *arguments],
        text=True,
        capture_output=True,
        timeout=5,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout)


def wait_ready(process: subprocess.Popen, client: Path, address: str, timeout: float) -> dict:
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Pace exited before readiness with code {process.returncode}; see app.log")
        try:
            info = inspect(client, address, "info")
            tree = inspect(client, address, "tree")
            nodes = tree["Tree"]["accesskit"]["nodes"]
            if any(node["properties"].get("label", "").startswith("Terminal pane ") for _, node in nodes):
                return info
        except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired) as error:
            last_error = error
        time.sleep(0.05)
    raise RuntimeError(f"Pace did not expose a terminal pane within {timeout:g}s: {last_error}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--app", type=Path, default=REPO / "target/debug" / ("pace" + SUFFIX))
    parser.add_argument("--client", type=Path, default=REPO / "target/debug" / ("pace-inspect" + SUFFIX))
    parser.add_argument("--output", type=Path, default=REPO / "artifacts/native", help="Parent directory for unique run artifacts")
    parser.add_argument("--ready-timeout", type=float, default=30)
    restoration = parser.add_mutually_exclusive_group()
    restoration.add_argument("--restore", dest="restore", action="store_true", help="Run the graceful close/relaunch regression (default)")
    restoration.add_argument("--no-restore-check", dest="restore", action="store_false", help="Skip the graceful close/relaunch regression")
    parser.set_defaults(restore=True)
    args = parser.parse_args()
    for binary in (args.app, args.client):
        if not binary.is_file():
            parser.error(f"Missing binary {binary}; build with cargo build --features inspection --locked")
    if args.ready_timeout <= 0:
        parser.error("--ready-timeout must be positive")
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8]
    output = args.output.resolve() / run_id
    data = output / "data"
    data.mkdir(parents=True)
    address = free_endpoint()
    environment = os.environ.copy()
    environment["EGUI_INSPECTION"] = address
    # Native inspection is portable; force X11 only on the Linux display adapter.
    if sys.platform.startswith("linux"):
        environment.pop("WAYLAND_DISPLAY", None)
    command = [str(args.app.resolve()), "--data-root", str(data), "--cwd", str(data), "--no-restore", "--size", "1180x760"]
    metadata = {"run_id": run_id, "address": address, "data_root": str(data), "output": str(output), "platform": platform.platform(), "python": sys.version, "app": str(args.app.resolve()), "client": str(args.client.resolve()), "command": command, "status": "starting", "features": ["inspection"], "binary_sha256": hashlib.sha256(args.app.read_bytes()).hexdigest(), "lock_sha256": hashlib.sha256((REPO / "Cargo.lock").read_bytes()).hexdigest()}
    try:
        metadata["compiler"] = subprocess.run(["rustc", "--version"], capture_output=True, text=True, timeout=5).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        metadata["compiler"] = None
    report_path = output / "run.json"
    process = None
    regression = None
    try:
        with (output / "app.log").open("w") as app_log:
            process = subprocess.Popen(command, env=environment, stdout=app_log, stderr=subprocess.STDOUT, start_new_session=True)
            metadata["pid"] = process.pid
            report_path.write_text(json.dumps(metadata, indent=2) + "\n")
            print(f"Native run {run_id}: {address}; artifacts: {output}", flush=True)
            metadata["inspection_info"] = wait_ready(process, args.client.resolve(), address, args.ready_timeout)
            regression_command = [sys.executable, str(REPO / "scripts/inspect-regression.py"), "--client", str(args.client.resolve()), "--app", str(args.app.resolve()), "--addr", address, "--data-root", str(data), "--output", str(output)]
            if args.restore:
                regression_command.append("--restore")
            regression = subprocess.Popen(regression_command, env=environment)
            metadata["regression_pid"] = regression.pid
            metadata["status"] = "running"
            report_path.write_text(json.dumps(metadata, indent=2) + "\n")
            result = regression.wait(timeout=240)
            metadata["status"] = "passed" if result == 0 else "failed"
            metadata["regression_exit_code"] = result
            return result
    except (OSError, RuntimeError, subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        metadata["status"] = "failed"
        metadata["error"] = str(error)
        print(f"Native verification failed: {error}; artifacts: {output}", file=sys.stderr)
        return 1
    finally:
        if regression is not None:
            stop_owned(regression,timeout=10)
        if process is not None:
            stop_owned(process)
        report_path.write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    raise SystemExit(main())
