// A small model of Neptune's workspace state for the in-page window. It follows
// the shapes of `crates/neptune-model` and the saved `workspaces.json`: a
// workspace owns a binary layout of panes, and every change is one action.

export type Axis = "vertical" | "horizontal";

export type Layout =
  | { kind: "pane"; pane: number }
  | {
      kind: "split";
      id: number;
      axis: Axis;
      ratio: number;
      first: Layout;
      second: Layout;
    };

/** Terminal colours: a palette token or an ANSI index. */
export type Tone = "fg" | "secondary" | "muted" | number;
export type Span = { t: string; c?: Tone; b?: boolean };

export type Line =
  /** The prompt's context line: where the shell is. */
  | { k: "ctx"; cwd: string; branch?: string; host?: string }
  /** A command as it was entered. `ok` is the previous command's result. */
  | { k: "cmd"; text: string; ok: boolean }
  | { k: "out"; spans: Span[] };

export interface Pane {
  id: number;
  title: string;
  cwd: string;
  branch?: string;
  /** SSH destination when the terminal runs on another machine. */
  remote?: string;
  lines: Line[];
  input: string;
  /** A prompt is waiting for input. False while a command is producing output. */
  prompt: boolean;
  /** A program that keeps running, such as `tail -f`, holds the terminal. */
  busy?: boolean;
  /** The previous command succeeded; the prompt reddens when it did not. */
  ok: boolean;
  status: "starting" | "running" | "exited";
}

export interface Workspace {
  id: number;
  name: string;
  cwd: string;
  remote?: string;
  layout: Layout;
  active: number;
}

export type Overlay =
  | { kind: "none" }
  | { kind: "palette"; query: string; selected: number; typed: boolean }
  | { kind: "settings" }
  | { kind: "ssh"; workspace: number | null; host: string; typed: boolean }
  | { kind: "menu"; workspace: number };

export interface State {
  workspaces: Workspace[];
  panes: Record<number, Pane>;
  active: number | null;
  sidebar: boolean;
  zoomed: boolean;
  overlay: Overlay;
  search: { open: boolean; query: string; typed: boolean };
  /** A terminal being carried by its header to another place. */
  drag: Drag | null;
  /** The last shortcut performed, shown briefly over the window. */
  hud: { keys: string; label: string; n: number } | null;
  nextId: number;
  /** Workspaces number themselves; their identity colour follows the id. */
  nextWorkspace: number;
}

export type Edge = "left" | "right" | "top" | "bottom";

/** Where a carried terminal lands. */
export type Destination =
  /** Against an edge of another pane. */
  | { kind: "beside"; pane: number; edge: Edge }
  /** In another pane's place, which takes the carried one's. */
  | { kind: "swap"; pane: number }
  | { kind: "workspace"; workspace: number };

export interface Drag {
  pane: number;
  /** Where the carried terminal's chip is, as CSS lengths within the stage. */
  x: string;
  y: string;
  to: Destination | null;
  /** The tour is carrying it, so the chip glides between positions. */
  scripted?: boolean;
}

export type Action =
  | { type: "load"; state: State }
  | { type: "drag"; drag: Drag | null }
  | { type: "drop"; pane: number; to: Destination }
  /** Lets go of a carried terminal, dropping it where it is held. */
  | { type: "release" }
  | { type: "split"; pane: number; axis: Axis }
  | { type: "closePane"; pane: number }
  | { type: "focus"; pane: number }
  | { type: "focusDirection"; direction: Direction }
  | { type: "ratio"; split: number; ratio: number }
  | { type: "zoom" }
  | { type: "newWorkspace"; name?: string; cwd?: string; branch?: string }
  | { type: "connect"; workspace: number | null; destination: string }
  | { type: "disconnect"; workspace: number }
  | { type: "selectWorkspace"; workspace: number }
  | { type: "moveWorkspace"; workspace: number; index: number }
  | { type: "closeWorkspace"; workspace: number }
  | { type: "movePane"; pane: number; workspace: number }
  | { type: "toggleSidebar" }
  | { type: "overlay"; overlay: Overlay }
  | { type: "search"; open: boolean; query?: string; typed?: boolean }
  | { type: "hud"; keys: string; label: string }
  | { type: "input"; pane: number; input: string }
  | { type: "commit"; pane: number }
  | { type: "print"; pane: number; lines: Line[] }
  /** Rewrites the last row, as a program does when it draws progress. */
  | { type: "amend"; pane: number; line: Line }
  | { type: "ready"; pane: number; ok?: boolean }
  | { type: "patch"; pane: number; patch: Partial<Pane> }
  | { type: "clear"; pane: number }
  | { type: "restart"; pane: number };

export type Direction = "left" | "right" | "up" | "down";

/** History is bounded, as in the app; the page keeps far less of it. */
const HISTORY = 240;

export const out = (text: string, c?: Tone, b?: boolean): Line => ({
  k: "out",
  spans: [{ t: text, c, b }],
});
export const row = (...spans: (Span | string)[]): Line => ({
  k: "out",
  spans: spans.map((span) => (typeof span === "string" ? { t: span } : span)),
});

export function panesOf(layout: Layout): number[] {
  return layout.kind === "pane"
    ? [layout.pane]
    : [...panesOf(layout.first), ...panesOf(layout.second)];
}

function replace(
  layout: Layout,
  pane: number,
  make: (leaf: Layout) => Layout | null,
): Layout | null {
  if (layout.kind === "pane") return layout.pane === pane ? make(layout) : layout;
  const first = replace(layout.first, pane, make);
  const second = replace(layout.second, pane, make);
  // Closing a pane gives its place to its sibling.
  if (!first) return second;
  if (!second) return first;
  return first === layout.first && second === layout.second
    ? layout
    : { ...layout, first, second };
}

function setRatio(layout: Layout, split: number, ratio: number): Layout {
  if (layout.kind === "pane") return layout;
  if (layout.id === split) return { ...layout, ratio };
  return {
    ...layout,
    first: setRatio(layout.first, split, ratio),
    second: setRatio(layout.second, split, ratio),
  };
}

/** A length inside the stage: a share of it plus a fixed offset in pixels. */
export interface Span1D {
  pct: number;
  px: number;
}
export interface Box {
  x: Span1D;
  y: Span1D;
  w: Span1D;
  h: Span1D;
}
export interface Divider extends Box {
  id: number;
  axis: Axis;
  /** The split's own area, which a drag measures against. */
  area: Box;
}

export const GUTTER = 6;

export const length = (span: Span1D) => `calc(${span.pct}% + ${span.px}px)`;
const FULL: Box = {
  x: { pct: 0, px: 0 },
  y: { pct: 0, px: 0 },
  w: { pct: 100, px: 0 },
  h: { pct: 100, px: 0 },
};

/**
 * Where each pane and divider sits. Lengths stay symbolic so the browser
 * resolves them, and panes glide when a ratio changes.
 */
export function arrange(layout: Layout): {
  panes: Map<number, Box>;
  dividers: Divider[];
} {
  const panes = new Map<number, Box>();
  const dividers: Divider[] = [];
  const half = GUTTER / 2;
  const visit = (node: Layout, box: Box) => {
    if (node.kind === "pane") {
      panes.set(node.pane, box);
      return;
    }
    const vertical = node.axis === "vertical";
    const start = vertical ? box.x : box.y;
    const length = vertical ? box.w : box.h;
    const r = node.ratio;
    const cut: Span1D = {
      pct: start.pct + length.pct * r,
      px: start.px + length.px * r,
    };
    const a: Span1D = { pct: length.pct * r, px: length.px * r - half };
    const b: Span1D = {
      pct: length.pct * (1 - r),
      px: length.px * (1 - r) - half,
    };
    const after: Span1D = { pct: cut.pct, px: cut.px + half };
    const gap: Span1D = { pct: cut.pct, px: cut.px - half };
    const thickness: Span1D = { pct: 0, px: GUTTER };
    if (vertical) {
      dividers.push({ id: node.id, axis: node.axis, area: box, ...box, x: gap, w: thickness });
      visit(node.first, { ...box, w: a });
      visit(node.second, { ...box, x: after, w: b });
    } else {
      dividers.push({ id: node.id, axis: node.axis, area: box, ...box, y: gap, h: thickness });
      visit(node.first, { ...box, h: a });
      visit(node.second, { ...box, y: after, h: b });
    }
  };
  visit(layout, FULL);
  return { panes, dividers };
}

/** The area a carried terminal would take: half of a pane, or all of it. */
export function dropArea(box: Box, to: Destination): Box {
  if (to.kind !== "beside") return box;
  const half = (span: Span1D): Span1D => ({ pct: span.pct / 2, px: span.px / 2 - GUTTER / 2 });
  const middle = (start: Span1D, span: Span1D): Span1D => ({
    pct: start.pct + span.pct / 2,
    px: start.px + span.px / 2 + GUTTER / 2,
  });
  switch (to.edge) {
    case "left":
      return { ...box, w: half(box.w) };
    case "right":
      return { ...box, x: middle(box.x, box.w), w: half(box.w) };
    case "top":
      return { ...box, h: half(box.h) };
    case "bottom":
      return { ...box, y: middle(box.y, box.h), h: half(box.h) };
  }
}

/** Exchanges two panes' places and keeps every split. */
function swapLeaves(layout: Layout, a: number, b: number): Layout {
  if (layout.kind === "pane") {
    return layout.pane === a
      ? { kind: "pane", pane: b }
      : layout.pane === b
        ? { kind: "pane", pane: a }
        : layout;
  }
  return {
    ...layout,
    first: swapLeaves(layout.first, a, b),
    second: swapLeaves(layout.second, a, b),
  };
}

/** The pane nearest to `pane` in a direction, by the layout's geometry. */
export function adjacent(
  layout: Layout,
  pane: number,
  direction: Direction,
): number | null {
  const unit = (span: Span1D) => span.pct / 100;
  const { panes } = arrange(layout);
  const from = panes.get(pane);
  if (!from) return null;
  const rect = (box: Box) => ({
    l: unit(box.x),
    t: unit(box.y),
    r: unit(box.x) + unit(box.w),
    b: unit(box.y) + unit(box.h),
  });
  const a = rect(from);
  let best: { id: number; overlap: number } | null = null;
  for (const [id, box] of panes) {
    if (id === pane) continue;
    const b = rect(box);
    const touches =
      direction === "left"
        ? Math.abs(b.r - a.l) < 1e-6
        : direction === "right"
          ? Math.abs(b.l - a.r) < 1e-6
          : direction === "up"
            ? Math.abs(b.b - a.t) < 1e-6
            : Math.abs(b.t - a.b) < 1e-6;
    if (!touches) continue;
    const overlap =
      direction === "left" || direction === "right"
        ? Math.min(a.b, b.b) - Math.max(a.t, b.t)
        : Math.min(a.r, b.r) - Math.max(a.l, b.l);
    if (overlap > 1e-6 && (!best || overlap > best.overlap)) best = { id, overlap };
  }
  return best?.id ?? null;
}

export const activeWorkspace = (state: State): Workspace | undefined =>
  state.workspaces.find((workspace) => workspace.id === state.active);

export const activePane = (state: State): Pane | undefined => {
  const workspace = activeWorkspace(state);
  return workspace ? state.panes[workspace.active] : undefined;
};

/** A workspace is named after its host when remote: `user@host` → `host`. */
export function remoteLabel(destination: string): string {
  const host = destination.replace(/^ssh:\/\//, "").split("@").pop() ?? "";
  return host || destination;
}

/** An OpenSSH destination: no spaces, no leading dash. */
export function validDestination(text: string): boolean {
  const destination = text.trim();
  return (
    destination.length > 0 &&
    destination.length <= 255 &&
    !destination.startsWith("-") &&
    !/[\s\p{Cc}]/u.test(destination)
  );
}

function shell(id: number, from: Partial<Pane>): Pane {
  return {
    id,
    title: from.remote ? "ssh" : "zsh",
    cwd: from.cwd ?? "~",
    branch: from.branch,
    remote: from.remote,
    lines: [],
    input: "",
    prompt: !from.remote,
    ok: true,
    status: from.remote ? "starting" : "running",
  };
}

function editWorkspace(
  state: State,
  id: number,
  edit: (workspace: Workspace) => Workspace,
): State {
  return {
    ...state,
    workspaces: state.workspaces.map((workspace) =>
      workspace.id === id ? edit(workspace) : workspace,
    ),
  };
}

function editPane(state: State, id: number, edit: (pane: Pane) => Pane): State {
  const pane = state.panes[id];
  return pane ? { ...state, panes: { ...state.panes, [id]: edit(pane) } } : state;
}

const owner = (state: State, pane: number): Workspace | undefined =>
  state.workspaces.find((workspace) => panesOf(workspace.layout).includes(pane));

function removeWorkspace(state: State, id: number): State {
  const index = state.workspaces.findIndex((workspace) => workspace.id === id);
  if (index < 0) return state;
  const panes = { ...state.panes };
  for (const pane of panesOf(state.workspaces[index].layout)) delete panes[pane];
  const workspaces = state.workspaces.filter((workspace) => workspace.id !== id);
  const next = workspaces[Math.min(index, workspaces.length - 1)];
  return {
    ...state,
    workspaces,
    panes,
    active: state.active === id ? (next?.id ?? null) : state.active,
    zoomed: state.active === id ? false : state.zoomed,
  };
}

function removePane(state: State, id: number, keepSession: boolean): State {
  const workspace = owner(state, id);
  if (!workspace) return state;
  const layout = replace(workspace.layout, id, () => null);
  if (!layout) {
    // A workspace's last terminal takes the workspace with it.
    const without = removeWorkspace(state, workspace.id);
    return keepSession ? { ...without, panes: { ...without.panes, [id]: state.panes[id] } } : without;
  }
  const remaining = panesOf(layout);
  let next = editWorkspace(state, workspace.id, (current) => ({
    ...current,
    layout,
    active: current.active === id ? remaining[remaining.length - 1] : current.active,
  }));
  if (!keepSession) {
    const panes = { ...next.panes };
    delete panes[id];
    next = { ...next, panes };
  }
  return { ...next, zoomed: remaining.length > 1 && next.zoomed };
}

export function reduce(state: State, action: Action): State {
  switch (action.type) {
    case "load":
      return action.state;

    case "drag":
      return { ...state, drag: action.drag };

    case "release": {
      const drag = state.drag;
      if (!drag) return state;
      return drag.to
        ? reduce(state, { type: "drop", pane: drag.pane, to: drag.to })
        : { ...state, drag: null };
    }

    case "drop": {
      const { pane, to } = action;
      const settled = { ...state, drag: null };
      if (to.kind === "workspace") {
        return reduce(settled, { type: "movePane", pane, workspace: to.workspace });
      }
      const workspace = owner(state, pane);
      // A drop where the terminal already is changes nothing.
      if (!workspace || to.pane === pane || owner(state, to.pane) !== workspace) return settled;
      if (to.kind === "swap") {
        return editWorkspace(settled, workspace.id, (current) => ({
          ...current,
          layout: swapLeaves(current.layout, pane, to.pane),
          active: pane,
        }));
      }
      const without = replace(workspace.layout, pane, () => null);
      if (!without) return settled;
      const leading = to.edge === "left" || to.edge === "top";
      const carried: Layout = { kind: "pane", pane };
      const layout = replace(without, to.pane, (leaf) => ({
        kind: "split",
        id: state.nextId,
        axis: to.edge === "left" || to.edge === "right" ? "vertical" : "horizontal",
        ratio: 0.5,
        first: leading ? carried : leaf,
        second: leading ? leaf : carried,
      }))!;
      return {
        ...editWorkspace(settled, workspace.id, (current) => ({
          ...current,
          layout,
          active: pane,
        })),
        nextId: state.nextId + 1,
      };
    }

    case "split": {
      const workspace = owner(state, action.pane);
      const source = state.panes[action.pane];
      if (!workspace || !source) return state;
      const pane = state.nextId;
      const split = state.nextId + 1;
      const layout = replace(workspace.layout, action.pane, (leaf) => ({
        kind: "split",
        id: split,
        axis: action.axis,
        ratio: 0.5,
        first: leaf,
        second: { kind: "pane", pane },
      }))!;
      return {
        ...editWorkspace(state, workspace.id, (current) => ({
          ...current,
          layout,
          active: pane,
        })),
        // A split starts in the source terminal's directory, on its machine.
        panes: { ...state.panes, [pane]: shell(pane, source) },
        zoomed: false,
        nextId: state.nextId + 2,
      };
    }

    case "closePane":
      return removePane(state, action.pane, false);

    case "focus": {
      const workspace = owner(state, action.pane);
      if (!workspace || workspace.active === action.pane) return state;
      return editWorkspace(state, workspace.id, (current) => ({
        ...current,
        active: action.pane,
      }));
    }

    case "focusDirection": {
      const workspace = activeWorkspace(state);
      if (!workspace) return state;
      const target = adjacent(workspace.layout, workspace.active, action.direction);
      return target === null
        ? state
        : editWorkspace(state, workspace.id, (current) => ({ ...current, active: target }));
    }

    case "ratio": {
      const ratio = Math.min(0.9, Math.max(0.1, action.ratio));
      return {
        ...state,
        workspaces: state.workspaces.map((workspace) => ({
          ...workspace,
          layout: setRatio(workspace.layout, action.split, ratio),
        })),
      };
    }

    case "zoom": {
      const workspace = activeWorkspace(state);
      if (!workspace) return state;
      const several = panesOf(workspace.layout).length > 1;
      return { ...state, zoomed: several && !state.zoomed };
    }

    case "newWorkspace": {
      const id = state.nextWorkspace;
      const pane = state.nextId;
      const cwd = action.cwd ?? "~";
      return {
        ...state,
        workspaces: [
          ...state.workspaces,
          {
            id,
            // Named after its folder; the home directory is "Home".
            name: action.name ?? (cwd === "~" ? "Home" : (cwd.split("/").pop() ?? cwd)),
            cwd,
            layout: { kind: "pane", pane },
            active: pane,
          },
        ],
        panes: { ...state.panes, [pane]: shell(pane, { cwd, branch: action.branch }) },
        active: id,
        zoomed: false,
        overlay: { kind: "none" },
        nextId: state.nextId + 1,
        nextWorkspace: state.nextWorkspace + 1,
      };
    }

    case "connect": {
      const destination = action.destination.trim();
      if (!validDestination(destination)) return state;
      if (action.workspace === null) {
        const id = state.nextWorkspace;
        const pane = state.nextId;
        return {
          ...state,
          workspaces: [
            ...state.workspaces,
            {
              id,
              name: remoteLabel(destination),
              cwd: "~",
              remote: destination,
              layout: { kind: "pane", pane },
              active: pane,
            },
          ],
          panes: { ...state.panes, [pane]: shell(pane, { cwd: "~", remote: destination }) },
          active: id,
          zoomed: false,
          overlay: { kind: "none" },
          nextId: state.nextId + 1,
          nextWorkspace: state.nextWorkspace + 1,
        };
      }
      // Connecting an existing workspace restarts its terminals on the host.
      const workspace = state.workspaces.find((w) => w.id === action.workspace);
      if (!workspace) return state;
      const panes = { ...state.panes };
      for (const pane of panesOf(workspace.layout)) {
        panes[pane] = shell(pane, { cwd: "~", remote: destination });
      }
      return {
        ...editWorkspace(state, workspace.id, (current) => ({
          ...current,
          remote: destination,
        })),
        panes,
        overlay: { kind: "none" },
      };
    }

    case "disconnect": {
      const workspace = state.workspaces.find((w) => w.id === action.workspace);
      if (!workspace) return state;
      const panes = { ...state.panes };
      for (const pane of panesOf(workspace.layout)) {
        panes[pane] = shell(pane, { cwd: workspace.cwd });
      }
      return {
        ...editWorkspace(state, workspace.id, (current) => ({
          ...current,
          remote: undefined,
        })),
        panes,
        overlay: { kind: "none" },
      };
    }

    case "selectWorkspace":
      return state.active === action.workspace ||
        !state.workspaces.some((workspace) => workspace.id === action.workspace)
        ? state
        : { ...state, active: action.workspace, zoomed: false };

    case "moveWorkspace": {
      const from = state.workspaces.findIndex((w) => w.id === action.workspace);
      const to = Math.min(state.workspaces.length - 1, Math.max(0, action.index));
      if (from < 0 || from === to) return state;
      const workspaces = [...state.workspaces];
      const [moved] = workspaces.splice(from, 1);
      workspaces.splice(to, 0, moved);
      return { ...state, workspaces };
    }

    case "closeWorkspace":
      return { ...removeWorkspace(state, action.workspace), overlay: { kind: "none" } };

    case "movePane": {
      const source = owner(state, action.pane);
      const target = state.workspaces.find((w) => w.id === action.workspace);
      // A terminal keeps its session, so it stays on its machine.
      if (!source || !target || source.id === target.id || source.remote !== target.remote) {
        return state;
      }
      const last = panesOf(source.layout).length === 1;
      const removed = removePane(state, action.pane, true);
      const split = removed.nextId;
      const moved = editWorkspace(removed, target.id, (current) => ({
        ...current,
        layout: replace(current.layout, current.active, (leaf) => ({
          kind: "split",
          id: split,
          axis: "vertical",
          ratio: 0.5,
          first: leaf,
          second: { kind: "pane", pane: action.pane },
        }))!,
        active: action.pane,
      }));
      return {
        ...moved,
        // Moving a workspace's last terminal follows the terminal.
        active: last ? target.id : moved.active,
        nextId: removed.nextId + 1,
      };
    }

    case "toggleSidebar":
      return { ...state, sidebar: !state.sidebar };

    case "overlay":
      return { ...state, overlay: action.overlay };

    case "search":
      return {
        ...state,
        search: {
          open: action.open,
          query: action.query ?? (action.open ? state.search.query : ""),
          typed: action.typed ?? false,
        },
      };

    case "hud":
      return {
        ...state,
        hud: { keys: action.keys, label: action.label, n: (state.hud?.n ?? 0) + 1 },
      };

    case "input":
      return editPane(state, action.pane, (pane) => ({ ...pane, input: action.input }));

    case "commit":
      return editPane(state, action.pane, (pane) => ({
        ...pane,
        lines: [
          ...pane.lines,
          { k: "ctx", cwd: pane.cwd, branch: pane.branch, host: pane.remote } as Line,
          { k: "cmd", text: pane.input, ok: pane.ok } as Line,
        ].slice(-HISTORY),
        input: "",
        prompt: false,
      }));

    case "print":
      return editPane(state, action.pane, (pane) => ({
        ...pane,
        lines: [...pane.lines, ...action.lines].slice(-HISTORY),
      }));

    case "amend":
      return editPane(state, action.pane, (pane) => ({
        ...pane,
        lines: [...pane.lines.slice(0, -1), action.line],
      }));

    case "ready":
      return editPane(state, action.pane, (pane) => ({
        ...pane,
        prompt: true,
        busy: false,
        status: "running",
        ok: action.ok ?? true,
      }));

    case "patch":
      return editPane(state, action.pane, (pane) => ({ ...pane, ...action.patch }));

    case "clear":
      return editPane(state, action.pane, (pane) => ({ ...pane, lines: [] }));

    case "restart":
      return editPane(state, action.pane, (pane) =>
        shell(pane.id, { cwd: pane.cwd, branch: pane.branch, remote: pane.remote }),
      );
  }
}

/** A minimal external store, so scripted steps read each change at once. */
export class Store {
  private listeners = new Set<() => void>();
  constructor(private state: State) {}
  get = (): State => this.state;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  dispatch = (action: Action) => {
    const next = reduce(this.state, action);
    if (next === this.state) return;
    this.state = next;
    for (const listener of this.listeners) listener();
  };
}
