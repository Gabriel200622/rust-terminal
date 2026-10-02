# Terminal interface direction

Pace's interface was rebuilt on 2026-09-30 around a native, Apple-style visual
language: one quiet window material, content that sits in it as rounded
surfaces, depth from elevation rather than borders, and a single accent reserved
for focus and selection. The terminal remains the dominant surface. Chrome earns
its space by being useful at a glance and then getting out of the way.

This replaces the earlier StarkIDE-derived interface. That design, its tokens and
its acceptance record remain in git history and under `artifacts/`; they are
historical evidence, not the current direction.

## Principles

- **Content first.** A single terminal has no header of its own; the toolbar
  names it. Pane headers, focus rings and dimming appear only when there is
  more than one terminal to tell apart.
- **Nothing covers or reflows the shell.** Search lives in the toolbar. Opening
  it, the command palette or a sheet never resizes a PTY. Status that belongs to
  a pane floats inside that pane; application messages sit in the window's top
  trailing corner, clear of the prompt.
- **The terminal owns its keys.** The focused terminal holds keyboard focus, so
  Tab, arrows and Escape reach the shell. Outside sheets and menus the toolkit
  never moves focus with Tab or the arrow keys; chrome is reached by pointer,
  shortcut, the command palette or assistive technology. Escape cancels a
  terminal drag, or leaves a sheet or a focused search field; otherwise it goes
  to the shell. A message never
  takes a key the shell is waiting for: it is dismissed by its button or the
  "Dismiss message" command, and by Escape only when no terminal is open. A key
  that closes a menu is not also sent to the shell, and Enter confirms a dialog
  only as a fresh press after the dialog is visible.
- **Quiet until needed.** Controls for a pane fade in on hover or focus. Motion
  is short (120–160 ms, ease-out) and used for state changes the pointer
  caused; keyboard-opened surfaces such as the command palette appear at once.
  A settled window does not repaint.
- **Every control is reachable and named.** Custom controls report native roles
  and stable labels; sheets are exposed as windows and keep focus inside them.

## Materials and colour

Tokens live in `src/theme.rs` (`Palette`, `metrics`). Use them rather than
literal colours.

| Token | Graphite | Dusk | Light | Use |
| --- | --- | --- | --- | --- |
| `chrome` | `#1c1c1f` | `#1e1c2b` | `#ececef` | Window material: toolbar and sidebar |
| `bg` | `#101012` | `#12111c` | `#ffffff` | Terminal content surface |
| `elevated` | `#29292d` | `#2b2840` | `#ffffff` | Sheets, menus, floating status |
| `fg` | `#ececf1` | `#e9e7f5` | `#1d1d1f` | Primary text |
| `secondary` | `#a0a0a8` | `#a29fb8` | `#5e5e66` | Supporting text, resting icons |
| `muted` | `#6d6d76` | `#6f6c87` | `#8e8e95` | Section labels, hints |

`control`, `hover`, `pressed`, `separator` and `border` are translucent white
(dark themes) or black (Light) so they read correctly on any material. Floating
surfaces carry a soft shadow and a one-pixel `border`; regions that share a
material are divided by a `separator` hairline or by space alone.

The accent is the user's choice of nine system-style colours (blue by default)
and resolves per theme. It marks the focused pane, selected rows, primary
buttons, switches, focus rings, search matches and the terminal cursor. Text on
the accent uses `on_accent`, which flips to dark ink for light accents. Terminal
selection is an opaque mix of accent and surface, so selected cells keep an
exact colour under any glyph. Red is reserved for destructive actions and
stopped terminals.

Workspace tiles take a stable identity colour from their workspace id: tinted
when idle, solid when selected. The identity colour is decoration only; state is
always also carried by text, a badge or position.

## Geometry

Radii are concentric: the window corner (16) equals the pane corner (10) plus
the gutter between them (6). Sheets use 14, controls and rows 8, menus 10.
Restored windows use the same transparent outer corners and hairline on Linux,
macOS and Windows. Maximized and fullscreen windows fill the screen with square
corners; restoring the window brings the rounded corners back.
The toolbar is 44 points tall and pane headers 30. Controls are 30 points high;
icon buttons keep a 28-point target around a 16-point glyph.

## Layout

- **Sidebar.** Full window height, like a native source list, and resizable from
  its trailing edge (saved on release; double-click restores the default). It
  hosts the window controls, the workspace list and a footer with "New
  workspace" and Preferences. Each row shows an identity tile, the name, a
  path that keeps its final directory, and either the pane count or, on hover,
  a "more" button with the same menu as a secondary click. A workspace
  connected over SSH shows a globe and its host in place of the path, and its
  menu offers "Disconnect from SSH" where a local one offers "Connect over
  SSH…". Double-click renames.
  Dragging a row reorders the workspaces: the row lifts onto the elevated
  material and follows the pointer, its neighbours ease aside to show where it
  will land, and it settles into that gap on release. Holding it at the list's
  edge scrolls a long list, Escape puts it back, and nothing is saved before
  release. "Move up" and "Move down" in the row menu and the command palette
  reorder without a pointer and take effect at once.
  While a terminal is carried, every other workspace's row on the same machine
  is a drop destination and takes the accent under the pointer.
  The sidebar yields to terminal content below 820 points of window width.
  Toggling it slides it in or out over 160 ms, from the button, the shortcut
  or the command palette alike. The window controls stay in place, the toggle
  travels between the sidebar's trailing edge and its place in the toolbar, and
  the terminals follow the sidebar's edge. Each shell is resized once, to the
  size it will rest at, and a toggle reversed midway turns around from where it
  is. Restored state and the width threshold take effect at once.
- **Toolbar.** Belongs to the content area and drags the window. Leading: the
  workspace name and the focused terminal's program and directory. Centre: a
  command field that opens the palette, replaced by the search field while
  searching. Trailing: find and split controls for the focused terminal. When
  the sidebar is hidden the toolbar also carries the window controls, the
  sidebar toggle and "New workspace". In narrow windows the command field
  collapses to an icon and search takes the title's room.
- **Window controls.** Close, minimize and maximize are three lights at the
  leading edge. Glyphs appear as the pointer approaches, and the lights turn
  neutral in an inactive window. Their accessible names are "Close window",
  "Minimize" and "Maximize".
- **Panes.** Rounded surfaces in the chrome, separated by a 6-point gutter that
  is also the split handle (a grip appears on hover; double-click evens the
  split). The surface takes the terminal's resolved background, so a program
  that changes it stays seamless. With several panes, each has a header with
  the program and directory (the host, for a remote terminal); the focused
  pane carries an accent ring and the others recede slightly. Zooming shows one pane and a "Zoomed" chip in the
  toolbar that restores the layout.
- **Moving a terminal.** A pane's header title is its handle. Dragging it lifts
  the terminal: its pane recedes, a chip with its name follows the pointer, and
  the pane under the pointer shows in the accent the area a drop would take.
  The nearest edge places the terminal beside that pane; the centre swaps the
  two, marked by a swap badge. The preview glides between areas and fades where
  it is released. Dropping on another workspace's sidebar row moves the
  terminal there, beside that workspace's roomiest pane and across its longer
  side, so arrivals fill a grid instead of narrowing one pane; the view stays
  where it was; moving a workspace's last terminal removes the workspace
  and follows the terminal. The shell keeps running throughout. A drop where
  the terminal already is changes nothing, and Escape or an opening sheet
  cancels the drag. A single terminal has no header, so it moves through the
  command palette's "Move terminal to …" commands.
- **Pane status.** Starting, exited, failed and resize-error states are shown by
  a centred capsule near the pane's bottom edge with one action. A successful
  exit is stated plainly; only failures use the problem colour. A remote
  terminal offers "Reconnect" where a local one offers "Restart". "Back to
  bottom" appears at the bottom trailing corner while scrolled into history.
- **Workspace creation.** New workspace immediately opens and selects a fresh
  shell in the home directory (`~`). Its name can be changed afterward from the
  sidebar or command palette. A new SSH workspace asks only for its host and
  is named after it.
- **Sheets.** Preferences, rename, SSH connection and close confirmation are
  centred modal sheets over a dimmed window; the dim follows the window's
  rounded shape. The confirming action sits at the trailing edge; destructive
  confirmations are red and are never the Enter default. Each close target has
  its own title and consequence ("Close terminal?", "Close workspace?",
  "Disconnect from SSH?", "Quit Pace?"). The SSH sheet asks only for a host,
  states that connecting an existing workspace restarts its terminals, and
  names the problem in place of that note while the host is unusable.
- **Preferences.** Grouped rows that apply immediately: theme previews drawn
  from each theme's own palette, accent dots, a window zoom percentage stepper,
  a font size stepper with its Ctrl+Shift+Plus/Minus and Ctrl+Shift+0 shortcuts
  (Command+Shift on macOS), a
  slider for line spacing, a segmented cursor style, and switches for boolean
  settings.
- **Command palette.** Every action with its shortcut, grouped when browsing and
  flat when filtered. Arrow keys move the highlight, Enter runs, and the pointer
  only takes the highlight when it moves. Commands that need a terminal are
  listed only when one is focused, and each captures its target when drawn.
- **Menus and tooltips.** Menus highlight the hovered row in the accent, show
  shortcuts at the trailing edge and mark destructive items in red. Tooltips
  name the control and its shortcut.

The default launch state is an actual shell. An empty window offers one action;
it does not show onboarding, sample data or decoration.

## Type and icon rhythm

Interface text is Geist in three weights: regular for body and values, medium
for names, labels and buttons, semibold for sheet titles and tiles. Sizes are
13 for body and names, 12–12.5 for secondary and control text, 11–11.5 for
section labels and paths, and 15 for sheet titles and the palette field.
Terminal text is JetBrains Mono. Preferences or Ctrl+Shift+Plus/Minus
(Command+Shift on macOS) changes its size; Ctrl+Shift+0 (Command+Shift+0 on
macOS) resets it to the default. App zoom
uses Ctrl+Plus/Minus (Command on macOS), with Ctrl+Equals as an unshifted Plus
alternative and Ctrl+0 to reset (Command on macOS). App zoom scales terminal
text and chrome together without changing the saved terminal font size. Window
zoom also appears under Appearance in Preferences and persists across launches,
whether changed there, by shortcut or from the command palette. Labels
use sentence case.

Icons are drawn natively on a 24-point grid with a 1.5-point stroke and rounded
ends, matching regular-weight text. Recolour an icon for state; do not swap
assets. Every icon-only control has a tooltip and an accessible name that match.

## Bundled fonts and notices

The application bundles unmodified static TrueType editions in `assets/fonts`,
fetched from the official Geist and JetBrains Mono repositories: Geist Regular,
Medium and SemiBold, and JetBrains Mono Regular and Bold. Their SIL Open Font
License notices and SHA-256 hashes are documented there. The bold terminal face
preserves the regular face's cell width. Static editions provide predictable
native metrics without a WOFF decoder; conversion or bundling must preserve each
font's copyright and complete license notice.

Installed Nerd, symbol and CJK fonts are loaded as fallbacks from known system
locations, once per process, for every interface weight and for the terminal.
They are not redistributed. This supports the development machine's shell
prompt without replacing the bundled Latin grid metrics; it does not establish
comprehensive shaping or font discovery on every platform. Missing glyphs and
complex-script behaviour need native verification.

## Screenshot review

Review the running native application, not a static mockup. For a routine UI
change, capture the affected states after the final build, including a narrow
window. Check that terminal type stays legible, the focused pane is
unambiguous, icon targets share a baseline, nothing clips, and overlays keep
focus. The terminal's output should hold more visual weight than the chrome.

Release visual acceptance requires fresh native captures after the last build,
covering single and three-pane layouts, workspace creation, Graphite and Light
preferences, search, the terminal menu, close confirmation, and 900×640 and
640×480 logical windows. A successful interaction report alone does not approve
the visual design. The evidence and remaining release gates are described in
[verification.md](verification.md).

### Review record: 2026-09-30 rebuild

Reviewed on Linux (X11 through the inspection harness, 1 px per logical point)
from fresh captures of the rebuilt interface: single pane, three panes, hidden
sidebar, zoom, search, command palette (browsing, filtered and without a
terminal), preferences in all three themes with a non-default accent, new
workspace, both menus, tooltips, close and quit confirmation, exited and failed
terminals, the error message, the empty window, and 900×640 and 640×480 windows.

Corrections made during that review:

- Search first floated over the pane and could cover a match; it moved into the
  toolbar.
- The error message first sat at the bottom centre and hid a failed pane's
  status; it moved to the top trailing corner.
- Menus stretched to the widest space offered; they now have a fixed width.
- A dialog's first field did not take the keyboard, because focus was requested
  during the sheet's hidden measuring pass.
- Escape that closed a menu was also delivered to the shell.
- A placeholder pane (starting or failed) drew a cursor; it no longer does.

An independent code review then found focus edge cases, all corrected: Tab or
an arrow pressed in the frame after the terminal regained focus could move
focus into the chrome; the palette re-requested focus every frame, which
interrupts input-method composition; same-named workspaces shared a palette row
identity; the search buttons handed the keyboard to the shell while their
tooltips promised Enter; a message swallowed one Escape meant for the shell; a
held Enter could confirm a dialog before it appeared; and double-click on the
split and sidebar handles was not sensed.

This record covers the recorded Linux states. Wayland input, macOS and Windows
still require native design review, as do the window controls' platform fit.
