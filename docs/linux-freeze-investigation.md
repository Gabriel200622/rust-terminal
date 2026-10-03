# Linux prerelease freeze investigation — 2026-10-03

This investigates [issue #5](https://github.com/zevem/neptune/issues/5) and an
owner report against the public `0.1.0-rc.1` AppImage. The entire window stopped
responding after time spent in other applications, recovered spontaneously,
froze again, and did not recover when resized or minimized/restored.

## Conclusion and limits

A whole-window stall was reproduced in an isolated Neptune instance on this
host under **XWayland**. Its main thread blocked in NVIDIA's Vulkan swapchain
image acquisition. Changing Linux presentation from `AutoVsync` to
`AutoNoVsync` removed that stall in the tested workload. PTY readers and parsers
were not the blocking threads.

The owner's original AppImage runs on **native Wayland**. Its two readable
captures showed the main thread waiting in the Wayland event loop, rather than
GPU acquisition. The final capture showed no outstanding frame callback, no
pending redraw, and no keyboard focus. The owner subsequently confirmed that
the search shortcut responded again. Those captures cannot establish the cause
of the earlier unresponsive state. The reproduced XWayland defect is fixed;
the original intermittent Wayland freeze remains unconfirmed.

## Build and host

- Public source: `v0.1.0-rc.1`,
  `316c932675b10d9d3d94fa31b63152d6df68e1a5`.
- Investigation base: `b624397cdc15d8c4a4fe9a1e324af3313a6be213`.
  The app entry point, app loop, input, window handling and dependency lockfile
  match the public tag in the relevant paths.
- Installed AppImage SHA-256:
  `8b04ee53593b5c6139e3992dab5cb434cf610c62f866cacac606a018cce7fa9e`.
- Installed executable SHA-256:
  `e1ee81f3ebe71c33ee302bdf2e35e67c22761f95bfb44bdb264b47b50257ea3b`.
- Ubuntu, GNOME 50.1 Wayland session; Linux 7.0.0-34-generic x86_64,
  glibc 2.43; hybrid Intel/NVIDIA RTX 4060 Laptop graphics;
  NVIDIA driver 595.91.07. Native Wayland scale was 1.66667.
- Local builds use pinned Rust 1.97.1 and the unchanged lockfile:
  eframe/egui 0.36.2, wgpu 30.0.1 and winit 0.30.13.
- Automated native/resource runs used the `inspection` feature and separate
  data roots, loopback endpoints and owned processes. The installed AppImage
  was only observed and attached for the owner's requested read-only captures.
  Stack arguments were suppressed; captures exclude terminal input and text.

## Reproduced blocking path

The old presentation setting repeatedly delayed inspection responses for
approximately one second per redraw; a sequence of UI and shell operations
took 7.3–8.5 seconds. A quiet repeat without a debugger or output producer
also exceeded the two-second request deadline. The main-thread stack was:

```text
ioctl
drmSyncobjTimelineWait
NVIDIA driver
ash::Device::acquire_next_image
wgpu_hal::vulkan::swapchain::native::acquire
wgpu::Surface::get_current_texture
egui_wgpu::Painter::paint_and_update_textures
eframe::WgpuWinitRunning::run_ui_and_paint
```

GPU acquisition runs synchronously on the event/UI thread. While it waits,
clicks, shortcuts and pane controls cannot be processed. Neptune's frame CPU
statistics do not include this later GPU wait, so a small frame CPU time does
not establish responsiveness.

The surface advertised `Fifo` and `Immediate`, without `Mailbox`. Increasing
the surface latency from one to two buffers still stalled. Forcing Mailbox
failed startup. Selecting the integrated adapter failed surface compatibility
on this host. These alternatives were discarded.

Linux now uses wgpu's `AutoNoVsync`, which selects Immediate on this surface and
retains a supported fallback on other surfaces. Other operating systems keep
the previous `AutoVsync` setting. This trades synchronized presentation for
responsiveness and can permit tearing on native X11. It does not introduce a
continuous repaint loop, change terminal processing, or upgrade dependencies.

An [upstream egui report](https://github.com/emilk/egui/issues/7561) describes
related idle/focus freezes with hybrid NVIDIA graphics on Linux, but explicitly
distinguishes Wayland. It is supporting context, not proof of this owner's
native Wayland failure.

## Verification

- Focused `cargo test -p neptune-terminal --bin neptune --locked`: both existing
  executable tests passed.
- Release build of the affected desktop and inspection-client targets passed.
- Three release XWayland idle/UI/shell cycles passed with the candidate:
  inspection tree responses took 2.48–8.66 ms. The complete sequences took
  305–322 ms, including an intentional 200 ms settling delay.
- Manually brought-forward native Wayland windows passed three cycles with
  both the candidate and original presentation modes. Focus metadata verified
  visibility during these checks. This is compatibility evidence, not a
  reproduction or repair of the intermittent Wayland freeze.
- The release integrated native harness passed workspace/pane controls, shell
  input, search, close cancellation, themes, narrow resize, and preservation of
  a 330-byte shell input buffer. Restoration was explicitly skipped in this run.
  Wide and 640×480 running-app captures were visually reviewed.
- Scoped release resource runs covered one/two panes, quiet, visible output and
  hidden output. Five-second quiet samples used 1.0–1.2% of one CPU core;
  visible output used 16.0–17.6%; hidden output with two panes used 1.2%.
  These are coarse process CPU observations. Inspection roundtrips and shell
  marker checks are not keyboard-to-display or PTY-to-screen measurements.
- An injected missing Wayland frame callback reproduced a different redraw
  starvation mechanism. A temporary winit recovery prototype addressed that
  synthetic case, but the original AppImage capture did not show that state.
  The prototype was excluded from the fix.

Evidence is retained locally in `/tmp/neptune-freeze-investigation/`:
`live-freeze/stack-readable.txt`, `live-freeze/callback-state.txt`,
`x11-febe7293/blocked-stack.txt`, `x11-f42ef804/report.json`,
`x11-c5a60716/report.json`, `x11-f4405806/report.json`,
`wayland-e344023d/report.json`, `wayland-134e861f/report.json`,
`resources-release/20261003T035932Z-70d82e24/report.json`, and
`native-release/20261003T043732Z-e0295da4/`. These local artifacts are not
published release acceptance records.

## Next capture if native Wayland freezes again

Confirm that the original window is visible and still rejects a search toggle
immediately before collecting a stack. Record frame-callback state, keyboard
focus and pending redraw alongside the stack. If GPU acquisition is blocked,
compare the candidate under the same conditions. If the loop is waiting with a
pending callback, investigate compositor/callback delivery. If it is waiting
with neither a callback nor pending redraw, inspect OS event delivery and
viewport visibility. Do not infer a deadlock from an ordinary idle event-loop
stack or collect terminal contents.
