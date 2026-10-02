#!/usr/bin/env python3
"""Linux/X11 adapter: inspect, capture, and drive an explicitly selected Neptune window.

This script does not start the app. On a Wayland desktop, launch Neptune with
WAYLAND_DISPLAY unset so its window is available to this X11 helper.
Requires Python 3, Pillow for screenshots, libX11, and libXtst for input.

Examples:
    python3 scripts/native-smoke.py info
    python3 scripts/native-smoke.py snapshot artifacts/native-main.png
    python3 scripts/native-smoke.py type 'printf "hello\\n"'
    python3 scripts/native-smoke.py key Return
    python3 scripts/native-smoke.py key ctrl+shift+t
    python3 scripts/native-smoke.py click 400 220
    python3 scripts/native-smoke.py resize 1100 760
"""

from __future__ import annotations

import argparse
import ctypes as C
import ctypes.util
import json
import os
from pathlib import Path
import sys
import time


Window = C.c_ulong
Display = C.c_void_p


class ClientMessageData(C.Union):
    _fields_ = [("bytes", C.c_char * 20), ("shorts", C.c_short * 10), ("longs", C.c_long * 5)]


class ClientMessage(C.Structure):
    _fields_ = [("type", C.c_int), ("serial", C.c_ulong), ("send_event", C.c_int),
               ("display", Display), ("window", Window), ("message_type", C.c_ulong),
               ("format", C.c_int), ("data", ClientMessageData)]


class XEvent(C.Union):
    _fields_ = [("client", ClientMessage), ("padding", C.c_long * 24)]


class KeyboardState(C.Structure):
    _fields_ = [("group", C.c_ubyte), ("locked_group", C.c_ubyte),
               ("base_group", C.c_ushort), ("latched_group", C.c_ushort),
               ("mods", C.c_ubyte), ("base_mods", C.c_ubyte),
               ("latched_mods", C.c_ubyte), ("locked_mods", C.c_ubyte),
               ("compat_state", C.c_ubyte), ("grab_mods", C.c_ubyte),
               ("compat_grab_mods", C.c_ubyte), ("lookup_mods", C.c_ubyte),
               ("compat_lookup_mods", C.c_ubyte), ("ptr_buttons", C.c_ushort)]


class XImage(C.Structure):
    _fields_ = [
        ("width", C.c_int),
        ("height", C.c_int),
        ("xoffset", C.c_int),
        ("format", C.c_int),
        ("data", C.c_void_p),
        ("byte_order", C.c_int),
        ("bitmap_unit", C.c_int),
        ("bitmap_bit_order", C.c_int),
        ("bitmap_pad", C.c_int),
        ("depth", C.c_int),
        ("bytes_per_line", C.c_int),
        ("bits_per_pixel", C.c_int),
        ("red_mask", C.c_ulong),
        ("green_mask", C.c_ulong),
        ("blue_mask", C.c_ulong),
        ("obdata", C.c_void_p),
    ]


class X11:
    def __init__(self) -> None:
        name = ctypes.util.find_library("X11")
        if not name:
            raise RuntimeError("libX11 is not installed")
        self.lib = C.CDLL(name)
        self._declare("XOpenDisplay", Display, [C.c_char_p])
        self._declare("XCloseDisplay", C.c_int, [Display])
        self._declare("XDefaultRootWindow", Window, [Display])
        self._declare("XQueryTree", C.c_int, [Display, Window, C.POINTER(Window), C.POINTER(Window), C.POINTER(C.POINTER(Window)), C.POINTER(C.c_uint)])
        self._declare("XFetchName", C.c_int, [Display, Window, C.POINTER(C.c_void_p)])
        self._declare("XFree", C.c_int, [C.c_void_p])
        self._declare("XGetGeometry", C.c_int, [Display, Window, C.POINTER(Window), C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(C.c_uint), C.POINTER(C.c_uint), C.POINTER(C.c_uint), C.POINTER(C.c_uint)])
        self._declare("XTranslateCoordinates", C.c_int, [Display, Window, Window, C.c_int, C.c_int, C.POINTER(C.c_int), C.POINTER(C.c_int), C.POINTER(Window)])
        self._declare("XGetImage", C.POINTER(XImage), [Display, Window, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_ulong, C.c_int])
        self._declare("XGetPixel", C.c_ulong, [C.POINTER(XImage), C.c_int, C.c_int])
        self._declare("XDestroyImage", C.c_int, [C.POINTER(XImage)])
        self._declare("XSetInputFocus", C.c_int, [Display, Window, C.c_int, C.c_ulong])
        self._declare("XGetInputFocus", C.c_int, [Display, C.POINTER(Window), C.POINTER(C.c_int)])
        self._declare("XRaiseWindow", C.c_int, [Display, Window])
        self._declare("XInternAtom", C.c_ulong, [Display, C.c_char_p, C.c_int])
        self._declare("XSendEvent", C.c_int, [Display, Window, C.c_int, C.c_long, C.POINTER(XEvent)])
        self._declare("XWarpPointer", C.c_int, [Display, Window, Window, C.c_int, C.c_int, C.c_uint, C.c_uint, C.c_int, C.c_int])
        self._declare("XFlush", C.c_int, [Display])
        self._declare("XSync", C.c_int, [Display, C.c_int])
        self._declare("XResizeWindow", C.c_int, [Display, Window, C.c_uint, C.c_uint])
        self._declare("XStringToKeysym", C.c_ulong, [C.c_char_p])
        self._declare("XKeysymToKeycode", C.c_uint, [Display, C.c_ulong])
        self._declare("XKeycodeToKeysym", C.c_ulong, [Display, C.c_uint, C.c_int])
        self._declare("XkbKeycodeToKeysym", C.c_ulong, [Display, C.c_uint, C.c_int, C.c_int])
        self._declare("XkbGetState", C.c_int, [Display, C.c_uint, C.POINTER(KeyboardState)])
        self.display = self.lib.XOpenDisplay(None)
        if not self.display:
            raise RuntimeError(f"Cannot open X11 display {os.environ.get('DISPLAY', '<unset>')}")
        self.root = self.lib.XDefaultRootWindow(self.display)
        self.xtst = None

    def _declare(self, name, restype, argtypes) -> None:
        function = getattr(self.lib, name)
        function.restype = restype
        function.argtypes = argtypes

    def close(self) -> None:
        if self.display:
            self.lib.XCloseDisplay(self.display)
            self.display = None

    def children(self, window: int) -> list[int]:
        root, parent = Window(), Window()
        children = C.POINTER(Window)()
        count = C.c_uint()
        if not self.lib.XQueryTree(self.display, window, C.byref(root), C.byref(parent), C.byref(children), C.byref(count)):
            return []
        try:
            return [children[i] for i in range(count.value)]
        finally:
            if children:
                self.lib.XFree(children)

    def title(self, window: int) -> str:
        value = C.c_void_p()
        if not self.lib.XFetchName(self.display, window, C.byref(value)) or not value:
            return ""
        try:
            return C.string_at(value).decode("utf-8", errors="replace")
        finally:
            self.lib.XFree(value)

    def find(self, title: str) -> int | None:
        pending = [(self.root, 0)]
        found = []
        while pending:
            current, depth = pending.pop(0)
            actual = self.title(current).casefold() if current != self.root else ""
            requested = title.casefold()
            matches = requested in actual
            # The default must not accidentally select another application's
            # window titled "Workspace" or "Workspaces".
            if requested == "neptune":
                matches = actual == "neptune" or actual.startswith(("neptune —", "neptune -")) or actual.endswith((" — neptune", " - neptune"))
            if current != self.root and matches:
                width, height = self.geometry(current)
                if width > 100 and height > 100:
                    found.append(current)
            if depth < 5:
                pending.extend((child, depth + 1) for child in self.children(current))
        if len(found) > 1:
            raise RuntimeError(f"Found {len(found)} matching windows; pass --window-id to select this run explicitly")
        return found[0] if found else None

    def geometry(self, window: int) -> tuple[int, int]:
        root, x, y = Window(), C.c_int(), C.c_int()
        width, height, border, depth = C.c_uint(), C.c_uint(), C.c_uint(), C.c_uint()
        if not self.lib.XGetGeometry(self.display, window, C.byref(root), C.byref(x), C.byref(y), C.byref(width), C.byref(height), C.byref(border), C.byref(depth)):
            raise RuntimeError("Cannot read window geometry")
        return width.value, height.value

    def origin(self, window: int) -> tuple[int, int]:
        x, y, child = C.c_int(), C.c_int(), Window()
        if not self.lib.XTranslateCoordinates(self.display, window, self.root, 0, 0, C.byref(x), C.byref(y), C.byref(child)):
            raise RuntimeError("Cannot translate window coordinates")
        return x.value, y.value

    def focus(self, window: int) -> None:
        self.lib.XRaiseWindow(self.display, window)
        # Let the window manager activate the window. Direct XSetInputFocus can
        # be reset to a compositor dummy window under GNOME/XWayland.
        event = XEvent()
        event.client.type = 33  # ClientMessage
        event.client.display = self.display
        event.client.window = window
        event.client.message_type = self.lib.XInternAtom(self.display, b"_NET_ACTIVE_WINDOW", 0)
        event.client.format = 32
        event.client.data.longs[0] = 1  # Application source
        self.lib.XSendEvent(self.display, self.root, 0, (1 << 20) | (1 << 19), C.byref(event))
        self.lib.XFlush(self.display)
        time.sleep(0.1)
        focused, revert = Window(), C.c_int()
        self.lib.XGetInputFocus(self.display, C.byref(focused), C.byref(revert))
        if focused.value != window:
            self.lib.XSetInputFocus(self.display, window, 2, 0)
            self.lib.XSync(self.display, 0)
            time.sleep(0.06)

    def input_library(self):
        if self.xtst is None:
            name = ctypes.util.find_library("Xtst")
            if not name:
                raise RuntimeError("libXtst is required for keyboard and pointer input")
            self.xtst = C.CDLL(name)
            self.xtst.XTestFakeKeyEvent.argtypes = [Display, C.c_uint, C.c_int, C.c_ulong]
            self.xtst.XTestFakeKeyEvent.restype = C.c_int
            self.xtst.XTestFakeButtonEvent.argtypes = [Display, C.c_uint, C.c_int, C.c_ulong]
            self.xtst.XTestFakeButtonEvent.restype = C.c_int
            self.xtst.XTestFakeMotionEvent.argtypes = [Display, C.c_int, C.c_int, C.c_int, C.c_ulong]
            self.xtst.XTestFakeMotionEvent.restype = C.c_int
        return self.xtst

    def keysym(self, name: str) -> int:
        aliases = {
            "ctrl": "Control_L", "control": "Control_L", "shift": "Shift_L",
            "alt": "Alt_L", "super": "Super_L", "meta": "Super_L",
            "enter": "Return", "return": "Return", "esc": "Escape", "escape": "Escape",
            "backspace": "BackSpace", "delete": "Delete", "tab": "Tab", "space": "space",
            "up": "Up", "down": "Down", "left": "Left", "right": "Right",
            "home": "Home", "end": "End", "pageup": "Prior", "pagedown": "Next",
        }
        normalized = aliases.get(name.lower(), name)
        if len(normalized) == 1 and 32 <= ord(normalized) <= 126:
            return ord(normalized)
        return self.lib.XStringToKeysym(normalized.encode("ascii"))

    def keycode(self, name: str) -> int:
        keysym = self.keysym(name)
        group = self.keyboard_group()
        candidates = [(level, code) for level in range(4) for code in range(8, 256)
                      if self.lib.XkbKeycodeToKeysym(self.display, code, group, level) == keysym]
        code = min(candidates)[1] if candidates else self.lib.XKeysymToKeycode(self.display, keysym)
        if not keysym or not code:
            raise RuntimeError(f"No X11 keycode for {name!r}")
        return code

    def keyboard_group(self) -> int:
        state = KeyboardState()
        if self.lib.XkbGetState(self.display, 0x0100, C.byref(state)) != 0:
            raise RuntimeError("Cannot read the active keyboard layout")
        return state.group

    def press(self, code: int, down: bool) -> None:
        if not self.input_library().XTestFakeKeyEvent(self.display, code, int(down), 0):
            raise RuntimeError("XTest rejected keyboard input")

    def chord(self, keys: str) -> None:
        names = keys.split("+")
        if not all(names):
            raise RuntimeError("Use a key name such as plus when '+' is the literal key")
        codes = [self.keycode(name) for name in names]
        for code in codes:
            self.press(code, True)
            self.lib.XSync(self.display, 0)
            time.sleep(0.15)
        for code in reversed(codes):
            self.press(code, False)
            self.lib.XSync(self.display, 0)
            time.sleep(0.04)
        self.lib.XSync(self.display, 0)

    def type_text(self, text: str, delay: float) -> None:
        # XTest uses the active keyboard map; only mapped ASCII input is accepted.
        # Validate everything first so an unsupported character cannot submit a
        # partial command to the shell.
        specials = {"\n": "Return", "\r": "Return", "\t": "Tab", " ": "space"}
        group = self.keyboard_group()
        prepared = []
        for char in text:
            if char not in specials and not 32 <= ord(char) <= 126:
                raise RuntimeError("type accepts mapped ASCII text; use the application's clipboard for Unicode")
            name = specials.get(char, char)
            keysym = self.keysym(name)
            code = self.keycode(name)
            levels = [self.lib.XkbKeycodeToKeysym(self.display, code, group, index) for index in range(4)]
            if not any(levels):
                levels = [self.lib.XkbKeycodeToKeysym(self.display, code, 0, index) for index in range(4)]
            if keysym not in levels:
                raise RuntimeError(f"Character {char!r} is not on the active keyboard layout")
            level = levels.index(keysym)
            prepared.append((code, bool(level & 1), bool(level & 2)))
        shift = self.keycode("shift")
        level3 = self.keycode("ISO_Level3_Shift") if any(entry[2] for entry in prepared) else 0
        for code, needs_shift, needs_level3 in prepared:
            if needs_shift:
                self.press(shift, True)
            if needs_level3:
                self.press(level3, True)
            if needs_shift or needs_level3:
                self.lib.XFlush(self.display)
                time.sleep(0.02)
            self.press(code, True)
            self.press(code, False)
            if needs_shift:
                self.press(shift, False)
            if needs_level3:
                self.press(level3, False)
            if needs_shift or needs_level3:
                self.lib.XFlush(self.display)
                time.sleep(0.02)
            self.lib.XFlush(self.display)
            if delay:
                time.sleep(delay)
        self.lib.XSync(self.display, 0)

    def click(self, window: int, x: int, y: int, button: int) -> None:
        width, height = self.geometry(window)
        if not (0 <= x < width and 0 <= y < height):
            raise RuntimeError(f"Click ({x}, {y}) is outside the {width}×{height} window")
        xtst = self.input_library()
        # Window-relative warping avoids the compositor transforming absolute
        # XTest motion twice on mixed-DPI/multiple-monitor XWayland desktops.
        self.lib.XWarpPointer(self.display, 0, window, 0, 0, 0, 0, x, y)
        self.lib.XSync(self.display, 0)
        xtst.XTestFakeButtonEvent(self.display, button, 1, 0)
        xtst.XTestFakeButtonEvent(self.display, button, 0, 0)
        self.lib.XSync(self.display, 0)

    def snapshot(self, window: int, destination: Path) -> None:
        try:
            from PIL import Image
        except ImportError as exc:
            raise RuntimeError("Pillow is required for PNG screenshots") from exc
        width, height = self.geometry(window)
        pointer = self.lib.XGetImage(self.display, window, 0, 0, width, height, C.c_ulong(-1).value, 2)
        if not pointer:
            raise RuntimeError("XGetImage could not capture the window")
        try:
            image = pointer.contents
            raw = C.string_at(image.data, image.bytes_per_line * image.height)
            common_rgb = (image.red_mask, image.green_mask, image.blue_mask) == (0xFF0000, 0x00FF00, 0x0000FF)
            if common_rgb and image.bits_per_pixel in (24, 32):
                mode = ("BGRX" if image.byte_order == 0 else "XRGB") if image.bits_per_pixel == 32 else ("BGR" if image.byte_order == 0 else "RGB")
                result = Image.frombytes("RGB", (width, height), raw, "raw", mode, image.bytes_per_line, 1)
            else:
                masks = [image.red_mask, image.green_mask, image.blue_mask]
                shifts = [(mask & -mask).bit_length() - 1 if mask else 0 for mask in masks]
                maximums = [mask >> shift for mask, shift in zip(masks, shifts)]
                pixels = bytearray(width * height * 3)
                for y in range(height):
                    for x in range(width):
                        pixel = self.lib.XGetPixel(pointer, x, y)
                        offset = (y * width + x) * 3
                        for channel, (mask, shift, maximum) in enumerate(zip(masks, shifts, maximums)):
                            pixels[offset + channel] = ((pixel & mask) >> shift) * 255 // maximum if maximum else 0
                result = Image.frombytes("RGB", (width, height), bytes(pixels))
            destination.parent.mkdir(parents=True, exist_ok=True)
            result.save(destination, "PNG")
        finally:
            self.lib.XDestroyImage(pointer)


def arguments():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--title", default="Neptune", help="case-insensitive window title substring")
    parser.add_argument("--window-id", type=lambda value: int(value, 0), help="explicit X11 window ID, decimal or hexadecimal")
    parser.add_argument("--wait", type=float, default=0, help="seconds to wait for a matching window")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("info", help="print the selected window and dimensions")
    snapshot = commands.add_parser("snapshot", help="save the native window framebuffer to PNG")
    snapshot.add_argument("path", type=Path)
    snapshot.add_argument("--settle", type=float, default=0.25, help="seconds to let the window paint")
    typing = commands.add_parser("type", help="type mapped ASCII text exactly as supplied")
    typing.add_argument("text")
    typing.add_argument("--delay", type=float, default=0.002, help="seconds between characters")
    key = commands.add_parser("key", help="press a key or chord, e.g. ctrl+shift+t")
    key.add_argument("chord")
    click = commands.add_parser("click", help="click a coordinate relative to the window content")
    click.add_argument("x", type=int)
    click.add_argument("y", type=int)
    click.add_argument("--button", type=int, default=1, choices=range(1, 10))
    resize = commands.add_parser("resize", help="resize the content area of the running window")
    resize.add_argument("width", type=int)
    resize.add_argument("height", type=int)
    return parser.parse_args()


def main() -> int:
    args = arguments()
    if not sys.platform.startswith("linux"):
        raise RuntimeError("native-smoke.py is the Linux/X11 input adapter; use native-harness.py for native inspection")
    if args.wait < 0 or args.wait > 60:
        raise RuntimeError("--wait must be between 0 and 60 seconds")
    x11 = X11()
    try:
        deadline = time.monotonic() + args.wait
        while True:
            window = args.window_id or x11.find(args.title)
            if window or time.monotonic() >= deadline:
                break
            time.sleep(0.1)
        if not window:
            raise RuntimeError(f"No X11 window matching {args.title!r}; launch Neptune with WAYLAND_DISPLAY unset")
        if args.command == "info":
            width, height = x11.geometry(window)
            print(json.dumps({"window": hex(window), "title": x11.title(window), "width": width, "height": height}))
            return 0
        x11.focus(window)
        if args.command == "snapshot":
            if not 0 <= args.settle <= 10:
                raise RuntimeError("--settle must be between 0 and 10 seconds")
            time.sleep(args.settle)
            x11.snapshot(window, args.path)
            print(str(args.path.resolve()))
        elif args.command == "type":
            if not 0 <= args.delay <= 1:
                raise RuntimeError("--delay must be between 0 and 1 second")
            x11.type_text(args.text, args.delay)
        elif args.command == "key":
            x11.chord(args.chord)
        elif args.command == "click":
            x11.click(window, args.x, args.y, args.button)
        elif args.command == "resize":
            if not (200 <= args.width <= 8192 and 150 <= args.height <= 8192):
                raise RuntimeError("Resize dimensions must be between 200×150 and 8192×8192")
            x11.lib.XResizeWindow(x11.display, window, args.width, args.height)
            x11.lib.XSync(x11.display, 0)
        return 0
    finally:
        x11.close()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (RuntimeError, OSError, ValueError) as error:
        print(f"native-smoke: {error}", file=sys.stderr)
        sys.exit(1)
