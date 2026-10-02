#!/usr/bin/env python3
"""Legacy Linux/X11 coordinate regression for an explicitly identified Neptune window.

Prefer native-harness.py for isolated launch and semantic widget verification.

Launch Neptune separately with WAYLAND_DISPLAY unset and an isolated config root,
then run this script. It does not launch, stop, or rebuild the application.
The workflow leaves Neptune running, returns its theme to Graphite, and cancels
the close confirmation. Screenshots and a machine-readable report are written
to --output. Coordinates are relative to the selected native window; modal
positions are inferred from screenshot differences rather than desktop pixels.

    env -u WAYLAND_DISPLAY target/debug/neptune --data-root /tmp/neptune-ui-qa --no-restore
    python3 scripts/ui-regression.py --data-root /tmp/neptune-ui-qa --window-id 0xWINDOW

Individual phases can be rerun with --phase after inspecting their captures.
An individual phase assumes the UI state created by the preceding phases.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import time
import tomllib


PHASES = ["create", "split", "input", "preferences", "light", "resize", "search", "close"]


class Regression:
    def __init__(self, args):
        self.args = args
        self.helper = Path(__file__).with_name("native-smoke.py")
        self.output = args.output.resolve()
        self.output.mkdir(parents=True, exist_ok=True)
        self.report = {"screenshots": [], "checks": [], "steps": [], "status": "running"}
        self.scale = args.scale or 1.0
        self.preferences_bounds = None

    def call(self, *arguments, pause=0.15):
        self.report["steps"].append(list(map(str, arguments)))
        command = [sys.executable, str(self.helper), "--window-id", str(self.args.window_id), *map(str, arguments)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())
        if pause:
            time.sleep(pause)
        return result.stdout.strip()

    def info(self):
        return json.loads(self.call("info", pause=0))

    def screenshot(self, name):
        path = self.output / f"{name}.png"
        self.call("snapshot", path, "--settle", 0.35, pause=0)
        self.report["screenshots"].append(str(path))
        self.save_report()
        print(f"Captured {path.name}", flush=True)
        return path

    def save_report(self):
        (self.output / "ui-regression.json").write_text(json.dumps(self.report, indent=2) + "\n")

    def key(self, chord, pause=0.2):
        self.call("key", chord, pause=pause)

    def type(self, text):
        self.call("type", text, "--delay", 0.02, pause=0.1)

    def shell(self, command, pause=0.35):
        self.type(command)
        self.key("Return", pause=pause)

    def click(self, x, y):
        self.call("click", round(x), round(y))

    def dimensions(self):
        info = self.info()
        return info["width"], info["height"]

    def infer_scale(self, screenshot):
        from PIL import Image
        image = Image.open(screenshot).convert("RGB")
        # The first broad horizontal border is the 46-logical-pixel title bar.
        known_borders = {(42, 42, 47), (52, 48, 63), (222, 222, 223)}
        width, height = image.size
        for y in range(20, min(180, height)):
            matches = sum(image.getpixel((x, y)) in known_borders for x in range(12, width - 12, 8))
            if matches >= (width - 24) / 8 * 0.75:
                # X11 desktop DPI scales often use thirds or quarters.
                return max(0.75, round(((y + 0.5) / 46) * 12) / 12)
        raise RuntimeError("Cannot infer desktop scale; pass --scale explicitly")

    def modal_bounds(self, before, after):
        from PIL import Image, ImageChops
        baseline = Image.open(before).convert("RGB")
        latest = Image.open(after).convert("RGB")
        if baseline.size != latest.size:
            raise RuntimeError("Window resized while locating a dialog")
        width, height = latest.size
        half_width = round(300 * self.scale)
        half_height = round(300 * self.scale)
        crop = (max(0, width // 2 - half_width), max(0, height // 2 - half_height),
                min(width, width // 2 + half_width), min(height, height // 2 + half_height))
        changed = ImageChops.difference(baseline.crop(crop), latest.crop(crop)).convert("L")
        mask = changed.point(lambda pixel: 255 if pixel > 8 else 0)
        bounds = mask.getbbox()
        if not bounds or bounds[2] - bounds[0] < 200 * self.scale:
            raise RuntimeError("Dialog could not be located in screenshot difference")
        return (crop[0] + bounds[0], crop[1] + bounds[1], crop[0] + bounds[2], crop[1] + bounds[3])

    def read_config(self):
        path = self.args.data_root / "config.toml"
        if not path.exists():
            raise RuntimeError(f"Expected isolated configuration at {path}")
        return tomllib.loads(path.read_text())

    def check(self, name, passed, detail):
        self.report["checks"].append({"name": name, "passed": bool(passed), "detail": detail})
        self.save_report()
        if not passed:
            raise RuntimeError(f"{name}: {detail}")
        print(f"Passed: {name}", flush=True)

    def create(self):
        self.key("Escape")
        self.screenshot("00-single-pane")
        state_path = self.args.data_root / "workspaces.json"
        before = json.loads(state_path.read_text())
        self.key("ctrl+shift+t", pause=0.8)
        self.screenshot("01-new-workspace")
        state = json.loads(state_path.read_text())
        created = next(workspace for workspace in state["workspaces"] if workspace["id"] == state["active"])
        self.check("workspace creation", len(state["workspaces"]) == len(before["workspaces"]) + 1 and created["id"] != before["active"] and Path(created["cwd"]) == Path.home(),
                   "New workspace must immediately select a new terminal at home")
        self.key("ctrl+shift+p")
        self.type("Rename workspace")
        self.key("Return")
        self.key("ctrl+a")
        self.type("Sandbox")
        self.key("Return", pause=0.8)
        self.screenshot("02-sandbox-workspace")
        state = json.loads(state_path.read_text())
        renamed = next(workspace for workspace in state["workspaces"] if workspace["id"] == created["id"])
        self.check("workspace renamed afterward", renamed["name"] == "Sandbox" and renamed["cwd"] == created["cwd"] and renamed["layout"] == created["layout"],
                   "Rename must preserve the new workspace directory and terminal")

    def split(self):
        self.key("ctrl+shift+d", pause=0.75)
        self.screenshot("03-split-right")
        self.key("ctrl+shift+e", pause=0.75)
        self.screenshot("04-split-below")

    def pane_coordinates(self):
        width, height = self.dimensions()
        # Default sidebar is 216 logical pixels and hides below 850 logical pixels.
        sidebar = 216 * self.scale if width / self.scale >= 850 else 0
        content_left = sidebar + 8 * self.scale
        usable = width - content_left - 8 * self.scale
        top = 46 * self.scale
        bottom = height - 25 * self.scale
        return [(content_left + usable * 0.25, top + (bottom - top) * 0.40),
                (content_left + usable * 0.75, top + (bottom - top) * 0.20),
                (content_left + usable * 0.75, top + (bottom - top) * 0.72)]

    def input(self):
        for index, (x, y) in enumerate(self.pane_coordinates(), start=1):
            self.click(x, y)
            self.shell(f"printf 'QA_PANE_{index}_OK\\n'")
        self.screenshot("05-shell-markers")
        marker = self.args.data_root / "interrupt-ok"
        marker.unlink(missing_ok=True)
        self.shell("sleep 30", pause=0.6)
        self.key("ctrl+c", pause=0.35)
        self.shell(f"printf 'QA_INTERRUPT_OK\\n' > '{marker}'", pause=0.6)
        self.check("Ctrl+C interrupts shell process", marker.exists(), "post-interrupt shell command must execute before sleep finishes")
        self.shell("printf 'QA_INTERRUPT_OK\\n'")
        self.screenshot("06-interrupted-process")

    def preferences(self):
        baseline = self.screenshot("07-before-preferences")
        self.key("ctrl+comma")
        dialog = self.screenshot("08-preferences-graphite")
        self.preferences_bounds = self.modal_bounds(baseline, dialog)
        (self.output / "preferences-bounds.json").write_text(json.dumps(self.preferences_bounds))

    def light(self):
        bounds = self.preferences_bounds
        stored = self.output / "preferences-bounds.json"
        if bounds is None and stored.exists():
            bounds = json.loads(stored.read_text())
        if bounds is None:
            raise RuntimeError("Run preferences phase before light, with the dialog open")
        left, top, right, bottom = bounds
        self.click(left + 182 * self.scale, top + 83 * self.scale)
        self.screenshot("09-preferences-light")
        self.check("Light theme selected", self.read_config().get("theme") == "light", "configuration theme should update after selection")
        self.key("Escape")
        self.screenshot("10-light-workspace")
        self.key("ctrl+comma")
        self.click(left + 45 * self.scale, top + 83 * self.scale)
        self.check("Graphite theme restored", self.read_config().get("theme") == "graphite", "restore theme for the remaining states")
        self.key("Escape")

    def resize(self):
        for width, height in ((900, 640), (640, 480)):
            self.call("resize", round(width * self.scale), round(height * self.scale), pause=0.5)
            self.screenshot(f"11-responsive-{width}x{height}")
        self.call("resize", round(1180 * self.scale), round(760 * self.scale), pause=0.5)

    def search(self):
        self.key("ctrl+shift+f")
        self.type("QA_INTERRUPT_OK")
        self.key("Return")
        self.screenshot("12-search-scrollback")
        self.key("Escape")

    def close(self):
        self.key("ctrl+shift+w")
        self.screenshot("13-close-confirmation")
        self.key("Escape")
        self.screenshot("14-close-cancelled")
        self.check("application remains open after cancelling close", bool(self.info().get("window")), "native window remains available")

    def run(self):
        initial = self.info()
        self.report["initial_window"] = initial
        initial_capture = self.screenshot("initial-window")
        if self.args.scale is None:
            self.scale = self.infer_scale(initial_capture)
        self.report["scale"] = self.scale
        print(f"Native scale: {self.scale:g}", flush=True)
        try:
            for phase in self.args.phase or PHASES:
                print(f"Phase: {phase}", flush=True)
                getattr(self, phase)()
            self.report["status"] = "passed"
        except Exception as error:
            self.report["status"] = "failed"
            self.report["error"] = str(error)
            try:
                self.screenshot("failure-state")
            except Exception:
                pass
            raise
        finally:
            self.save_report()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--window-id", type=lambda value: int(value,0), required=True, help="Exact X11 window ID belonging to this run")
    parser.add_argument("--data-root", "--config-root", dest="data_root", type=Path, required=True, help="Exact --data-root passed to Neptune")
    parser.add_argument("--output", type=Path, default=Path("artifacts/ui-regression"))
    parser.add_argument("--scale", type=float, help="native pixels per logical UI pixel; inferred from title-bar border otherwise")
    parser.add_argument("--phase", choices=PHASES, action="append", help="run selected phases instead of the complete sequence")
    args = parser.parse_args()
    if args.scale is not None and not 0.5 <= args.scale <= 4:
        parser.error("--scale must be between 0.5 and 4")
    Regression(args).run()


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"ui-regression: {error}", file=sys.stderr)
        sys.exit(1)
