# Native widgets

Apply [docs/design.md](../../docs/design.md) when changing visual hierarchy,
spacing, typography or controls. Keep terminal output dominant and chrome quiet;
use the existing theme tokens, outline icons and shared controls in `helpers.rs`
before adding a new visual pattern. `../theme.rs`, `../icons.rs` and
`../platform/fonts.rs` supply the shared visual system.

Widgets consume view data and emit targeted `Action`s rather than borrowing
`&mut App`. Overlay/input ownership is explicit in `UiState`/`OverlayState`;
retain dialog targets and preserve cancellation and terminal focus restoration.

Use stable semantic egui IDs across hover/press/release and responsive hiding.
A neighboring control disappearing must not transfer keyboard focus or change
what Enter activates. Give icon-only controls matching tooltips and accessible
names; expose stable pane/field labels for inspection. Narrow windows need
contained, scrollable overlays with reachable close/actions, not clipped forms.

For control/helper logic, select the affected test in `chrome.rs` or `helpers.rs`:

```sh
cargo test -p neptune-terminal --lib ui:: --locked
```

Review fresh native captures after the final build, using
[scripts/AGENTS.md](../../scripts/AGENTS.md). Check the changed control's normal,
focused and open/closed states as applicable, relevant themes, single/split panes
and a narrow window. Inspect text/icon alignment, terminal legibility, focus
clarity and clipping. For motion/timing, observe the running transition.
Synthetic egui tests prove identity/logic; they do not approve native appearance.
