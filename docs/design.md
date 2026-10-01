# Terminal interface direction

The interface borrows StarkIDE's visual rhythm and neutral palette. Its layout is
purpose-built for terminal work: the terminal remains the dominant surface, and
workspace navigation and controls take a small, predictable amount of space.

## Reference tokens

The reference is the local StarkIDE desktop application's `src/index.css`,
`components/ui/button.tsx`, `components/TitleBar/TitleBar.tsx`,
`components/TerminalPaneHeader.tsx`, and `components/WorkshopSidebar.tsx`.

| Role | StarkIDE token | Intended use |
| --- | --- | --- |
| Canvas | `#0f0f11` | Terminal background |
| Surface | `#171719` | Sidebar, pane header, settings |
| Hover | `#222226` | Quiet interaction feedback |
| Border | `#303036` | Inputs and stronger boundaries |
| Primary text | `#f5f5f7` | Names and editable values |
| Secondary text | `#a1a1aa` | Paths and supporting labels |
| Muted text | `#6e6e76` | Inactive icons and ambient metadata |
| Lavender accent | `#b9acf2` | Terminal-specific focus and selection |

Lavender is an original terminal accent rather than StarkIDE's signature cyan.
Use it sparingly: a selected workspace, focused pane marker, or focus ring. Most
chrome remains neutral. A selected fill may use roughly 10% accent opacity. Pane
borders should be quieter than controls: white at 7–10% opacity is sufficient.

StarkIDE uses an 8px base radius, deriving approximately 5, 6, 8, 11, and 14px.
Use 11–12px for workspace rows and dialogs, 8px for inputs and compact panels,
and circular hit areas for icon-only controls. Avoid nested rounded cards.

## Layout and hierarchy

- A 40–44px title bar contains a small terminal mark, the active workspace name,
  and the essential split, search, and new-session controls. Native window
  controls follow the host platform's conventions.
- A collapsible sidebar is about 220px wide. Its workspace list has 8px outer
  padding, 4px gaps, and 40–46px rows. Labels identify actual sessions; secondary
  paths are shown only when they help distinguish workspaces.
- Terminal panes use a flat grid with a narrow divider or an 8px gutter. A 28px
  pane header names the shell or session and holds a compact focus indicator.
  Split and close controls appear in the focused or hovered header.
- Each terminal gets 12px padding. With one pane, the terminal should feel like
  one continuous working surface rather than a floating card.
- A 22–24px footer can show the active path, terminal dimensions, and a quiet
  shell state. It does not contain marketing copy or decorative status badges.
- Search is a small attached field. Settings and the command palette are
  focused overlays with concise labels and keyboard hints.

The default launch state is an actual shell. Useful empty space is better than
invented activity, sample metrics, onboarding paragraphs, or a dashboard.

## Type and icon rhythm

The reference uses Geist Variable for interface text and JetBrains Mono
Variable for terminal text. Favor 12–13px UI text, 11px pane names and secondary
metadata, and a configurable 14–15px terminal size. Use medium weight for
workspace names and regular weight for surrounding controls. Avoid uppercase
labels except where genuinely conventional.

Icons follow Lucide's simple outlines: 14–16px visual size, roughly 1.75px
strokes, rounded caps, and generous 28–32px hit areas. Draw the native icons
from a coherent small set rather than relying on unrelated Unicode symbols.
Every icon-only control needs a tooltip and an accessible name.

## Local font sources and notices

StarkIDE's installed, licensed font files are available locally:

- `node_modules/@fontsource-variable/geist/files/geist-latin-wght-normal.woff2`
  and the neighboring `LICENSE`: SIL Open Font License 1.1, copyright the Geist
  Project Authors.
- `node_modules/@fontsource-variable/jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2`
  and the neighboring `LICENSE`: SIL Open Font License 1.1, copyright the
  JetBrains Mono Project Authors.

Paths above are relative to
`/home/biggabo/Documents/Projects/starkide/starkide`. Native renderers that only
accept OpenType/TrueType need the corresponding font format. Conversion or
bundling must preserve each font's copyright and complete license notice.

The application bundles unmodified static TrueType editions in `assets/fonts`,
fetched from the official Geist and JetBrains Mono repositories: Geist Regular,
JetBrains Mono Regular, and JetBrains Mono Bold. Their accompanying notices and
SHA-256 hashes are documented there. The bold terminal face preserves the regular
face's cell width. The static editions provide predictable native font metrics
without a WOFF decoder.

Installed Nerd, symbol, and CJK fonts are loaded as fallbacks from known system
locations, once per process. They are not redistributed. This supports the
development machine's shell prompt without replacing the bundled Latin grid
metrics; it does not establish comprehensive shaping or font discovery on every
platform. Missing glyphs and complex-script behavior need native verification.

Native-format alternatives on the development machine include
`/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf`,
`/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf`, and
`/usr/share/fonts/opentype/cantarell/Cantarell-VF.otf`. Their Debian package
copyright files contain the OFL notices. System-installed proprietary fonts
are not suitable bundled assets.

## Screenshot review

Review the running native application, not a static mockup. For a routine UI
change, capture affected states after the final build, including a smaller
window. Release visual acceptance includes a single pane, a split workspace and
a settings overlay as well. Check
that terminal type remains legible, pane headers align, icon targets share a
baseline, controls never crowd window buttons, and overlays preserve focus.
The terminal's output should hold more visual weight than the application's
chrome. Correct clipping, overly bright boundaries, ambiguous focused panes,
and excessively dense secondary labels before accepting the interface.

The implementation uses a 46px title bar, 34px pane headers, a 216px sidebar,
and consistent 28px icon targets. The sidebar collapses at smaller window widths
so shell content retains priority. Custom outline icons share a 1.5px stroke;
their accessible names match their tooltips.

The first native review identified excessively long shell titles and missing
prompt glyphs. Pane headers now show the shell and a compact path, and installed
Nerd fonts supply the development shell's symbols. An active pane uses a small
lavender focus marker; inactive boundaries remain neutral. Workspace paths
truncate on path segments so the final directory stays identifiable.

The overlay review identified cramped content padding and generic gray controls.
Preferences now use 16px inner padding, outlined editable controls, aligned label
and value columns, and restrained theme choices. A later native capture caught
a scroll-window regression: a zero default height reduced Preferences to a
122px strip and clipped almost every control. The window now has an intentional
initial height capped to the viewport, with scrolling for smaller windows. Both
full-size and narrow captures must show usable controls after that correction.
Primary actions have a clear position without adding decorative panels.

The responsive interaction review also caught keyboard focus being reused when
sidebar controls disappeared and returned. Stable semantic widget identities and
an explicit search-input identity now preserve the intended target across layout
changes. A regression test verifies that hiding a neighboring control keeps Enter
assigned to the focused action rather than activating another button.

Release visual acceptance requires fresh native captures after the last build, covering
single and three-pane layouts, workspace creation, Graphite and Light preferences,
search, close confirmation, and 900×640/640×480 logical windows. Review hierarchy,
alignment, typography, whitespace, color balance, density, responsiveness, and
visual character together. A successful interaction report alone does not approve
the visual design. The evidence and remaining release gates are described in
[verification.md](verification.md).

The final Linux native review on 2026-09-30 accepted 23 fresh captures after the
18-check interaction workflow passed. Terminal output retains the strongest
hierarchy; headers, controls, and metadata stay quiet. Icon baselines, pane
padding, and overlay columns align consistently. Graphite and Light preserve the
same geometry and restrained lavender focus treatment. At 640×480, the sidebar
collapses and Preferences uses a bounded scroll viewport with an accessible close
control. The unchecked Blink outline now remains visible in both themes.

The final resize captures show ordinary command history and a clean redrawn active
prompt, and the long typed buffer executes intact. Search preserves workspace
focus and uses coherent outline arrows and subdued match highlights. The restored
workspace capture contains no accidental path tooltip. No further visual
restructuring was required after these corrections. This acceptance covers the
recorded Linux states; other platforms still require native design review.
