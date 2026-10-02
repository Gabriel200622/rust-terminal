"""Portable tests for run isolation, semantic readiness and owned cleanup."""

import importlib.util
from pathlib import Path
import socket
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location("native_harness", Path(__file__).with_name("native-harness.py"))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

smoke_spec = importlib.util.spec_from_file_location("native_smoke", Path(__file__).with_name("native-smoke.py"))
smoke = importlib.util.module_from_spec(smoke_spec)
smoke_spec.loader.exec_module(smoke)


class NativeHarnessTests(unittest.TestCase):
    def test_endpoint_is_loopback_and_avoids_other_live_run(self):
        first = harness.free_endpoint()
        host, port = first.rsplit(":", 1)
        with socket.socket() as active_run:
            active_run.bind((host, int(port)))
            self.assertTrue(harness.free_endpoint().startswith("127.0.0.1:"))
            self.assertNotEqual(harness.free_endpoint(), first)

    def test_cleanup_stops_only_owned_process(self):
        command = [sys.executable, "-c", "import time; time.sleep(60)"]
        owned = subprocess.Popen(command)
        unrelated = subprocess.Popen(command)
        try:
            harness.stop_owned(owned)
            self.assertIsNotNone(owned.poll())
            self.assertIsNone(unrelated.poll())
        finally:
            harness.stop_owned(owned)
            harness.stop_owned(unrelated)

    def test_readiness_requires_semantic_terminal_identity(self):
        process = Mock()
        process.poll.return_value = None
        unrelated = {"Tree": {"accesskit": {"nodes": [[1, {"properties": {"label": "Unrelated widget"}}]]}}}
        ready = {"Tree": {"accesskit": {"nodes": [[2, {"properties": {"label": "Terminal pane 51"}}]]}}}
        with patch.object(harness, "inspect", side_effect=[{"info": 1}, unrelated, {"info": 2}, ready]) as inspect, patch.object(harness.time, "sleep"):
            self.assertEqual(harness.wait_ready(process, Path("client"), "127.0.0.1:51234", 1), {"info": 2})
            self.assertEqual(inspect.call_count, 4)
            self.assertTrue(all(call.args[1] == "127.0.0.1:51234" for call in inspect.call_args_list))

    def test_exited_process_cannot_be_ready_on_reused_endpoint(self):
        process = Mock()
        process.poll.return_value = 1
        process.returncode = 1
        with patch.object(harness, "inspect") as inspect:
            with self.assertRaisesRegex(RuntimeError, "exited before readiness"):
                harness.wait_ready(process, Path("client"), "127.0.0.1:51234", 1)
            inspect.assert_not_called()

    def test_linux_adapter_refuses_ambiguous_windows(self):
        x11 = smoke.X11.__new__(smoke.X11)
        x11.root = 1
        x11.children = lambda window: [2, 3] if window == 1 else []
        x11.title = lambda window: "Neptune" if window in [2, 3] else ""
        x11.geometry = lambda window: (1180, 760)
        with self.assertRaisesRegex(RuntimeError, "pass --window-id"):
            x11.find("Neptune")

    def test_linux_adapter_ignores_similarly_named_foreign_window(self):
        x11 = smoke.X11.__new__(smoke.X11)
        x11.root = 1
        x11.children = lambda window: [2, 3] if window == 1 else []
        x11.title = lambda window: {2:"Workspaces", 3:"Neptune"}.get(window, "")
        x11.geometry = lambda window: (1180, 760)
        self.assertEqual(x11.find("Neptune"), 3)


if __name__ == "__main__":
    unittest.main()
