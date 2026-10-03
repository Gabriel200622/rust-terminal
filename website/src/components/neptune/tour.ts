// The scripted demonstration that plays in the hero window. Each chapter
// drives the same actions a visitor can perform, so the tour and the window
// never disagree about what Neptune does.

import {
  Store,
  activeWorkspace,
  arrange,
  length,
  out,
  place,
  row,
  type Action,
  type Box,
  type Destination,
  type Line,
  type State,
} from "./model";
import { CARGO_TEST, GIT_LOG, TEST_COUNT } from "./shell";

const ctx = (cwd: string, branch?: string): Line => ({ k: "ctx", cwd, branch });
const cmd = (text: string): Line => ({ k: "cmd", text, ok: true });

/** Three workspaces, two of them in a folder, as a restored session opens them. */
export const BASE: State = {
  workspaces: [
    {
      id: 1,
      name: "neptune",
      cwd: "~/code/neptune",
      group: 1,
      layout: place(1),
      active: 1,
    },
    {
      id: 2,
      name: "api",
      cwd: "~/code/api",
      group: 1,
      layout: {
        kind: "split",
        id: 10,
        axis: "vertical",
        ratio: 0.5,
        first: place(2),
        second: place(3),
      },
      active: 3,
    },
    {
      id: 3,
      name: "notes",
      cwd: "~/notes",
      layout: place(4),
      active: 4,
    },
  ],
  groups: [{ id: 1, name: "Work", collapsed: false }],
  order: [
    { kind: "group", id: 1 },
    { kind: "workspace", id: 3 },
  ],
  alerts: [],
  shell: "zsh",
  panes: {
    1: {
      id: 1,
      title: "zsh",
      cwd: "~/code/neptune",
      branch: "main",
      lines: [],
      input: "",
      prompt: true,
      ok: true,
      status: "running",
    },
    2: {
      id: 2,
      title: "bun",
      cwd: "~/code/api",
      branch: "main",
      lines: [
        ctx("~/code/api", "main"),
        cmd("bun run dev"),
        row({ t: "$ bun --watch src/index.ts", c: "muted" }),
        row({ t: "ready", c: 2 }, " on http://localhost:3000"),
      ],
      input: "",
      prompt: false,
      busy: true,
      ok: true,
      status: "running",
    },
    3: {
      id: 3,
      title: "zsh",
      cwd: "~/code/api",
      branch: "main",
      lines: [
        ctx("~/code/api", "main"),
        cmd("git status"),
        out("On branch main"),
        out("nothing to commit, working tree clean"),
      ],
      input: "",
      prompt: true,
      ok: true,
      status: "running",
    },
    4: {
      id: 4,
      title: "zsh",
      cwd: "~/notes",
      lines: [ctx("~/notes"), cmd("ls"), out("ideas.md  today.md")],
      input: "",
      prompt: true,
      ok: true,
      status: "running",
    },
  },
  active: 1,
  sidebar: true,
  zoomed: false,
  overlay: { kind: "none" },
  search: { open: false, query: "", typed: false },
  drag: null,
  hud: null,
  nextId: 100,
  nextWorkspace: 4,
};

const BUILDING = row({ t: "   Compiling", c: 2, b: true }, " neptune-terminal v0.1.0");
const BUILT = row({ t: "    Finished", c: 2, b: true }, " `release` profile [optimized] target(s) in 41.07s");

const log = (time: string, level: "INFO" | "WARN" | "ERROR", text: string): Line =>
  row(
    { t: `${time} `, c: "muted" },
    { t: level.padEnd(5), c: level === "INFO" ? 4 : level === "WARN" ? 3 : 1, b: level !== "INFO" },
    ` ${text}`,
  );

const LOG: Line[] = [
  log("12:04:07", "INFO", "listening on :3000"),
  log("12:04:09", "INFO", "GET /health 200 2ms"),
  log("12:04:12", "WARN", "slow query users 412ms"),
  log("12:04:15", "INFO", "POST /sessions 201 18ms"),
  log("12:04:21", "ERROR", "upstream timeout billing"),
  log("12:04:22", "WARN", "retry 1/3 billing"),
  log("12:04:24", "INFO", "GET /users/42 200 6ms"),
];
const LOG_LATER: Line[] = [
  log("12:04:29", "WARN", "retry 2/3 billing"),
  log("12:04:31", "INFO", "POST /invoices 201 44ms"),
];

interface Script {
  d: (action: Action) => void;
  get: () => State;
  wait: (ms: number) => Promise<void>;
  /** Narrow windows have no sidebar and room for fewer panes. */
  compact: boolean;
}

const active = (s: Script) => activeWorkspace(s.get())!.active;

async function type(s: Script, pane: number, text: string) {
  for (let index = 1; index <= text.length; index++) {
    s.d({ type: "input", pane, input: text.slice(0, index) });
    await s.wait(38 + ((index * 37) % 46));
  }
}

/** Runs the typed command, printing its output a line at a time. */
async function enter(
  s: Script,
  pane: number,
  lines: Line[],
  { gap = 0, running = false }: { gap?: number; running?: boolean } = {},
) {
  await s.wait(260);
  s.d({ type: "commit", pane });
  if (gap === 0) {
    s.d({ type: "print", pane, lines });
  } else {
    for (const line of lines) {
      await s.wait(gap);
      s.d({ type: "print", pane, lines: [line] });
    }
  }
  if (running) s.d({ type: "patch", pane, patch: { busy: true } });
  else s.d({ type: "ready", pane });
}

/** Shows the shortcut that performs the next step. */
async function key(s: Script, keys: string, label: string) {
  s.d({ type: "hud", keys, label });
  await s.wait(420);
}

/** Carries a terminal by its tab to an edge of another place. */
async function carry(
  s: Script,
  pane: number,
  to: Extract<Destination, { kind: "beside" }>,
) {
  const boxes = arrange(activeWorkspace(s.get())!.layout).panes;
  const from = boxes.get(pane);
  const target = boxes.get(to.pane);
  if (!from || !target) return;
  const at = (box: Box, fx: number, fy: number, dy = 0) => ({
    x: length({ pct: box.x.pct + box.w.pct * fx, px: box.x.px + box.w.px * fx }),
    y: length({ pct: box.y.pct + box.h.pct * fy, px: box.y.px + box.h.px * fy + dy }),
  });
  const [fx, fy] = {
    left: [0.12, 0.45],
    right: [0.8, 0.45],
    top: [0.4, 0.12],
    bottom: [0.4, 0.78],
  }[to.edge];
  s.d({ type: "focus", pane });
  s.d({ type: "drag", drag: { pane, ...at(from, 0.08, 0, 4), to: null, scripted: true } });
  await s.wait(500);
  s.d({ type: "drag", drag: { pane, ...at(target, fx, fy), to: null, scripted: true } });
  await s.wait(480);
  s.d({ type: "drag", drag: { pane, ...at(target, fx, fy), to, scripted: true } });
  await s.wait(850);
  s.d({ type: "drop", pane, to });
  await s.wait(1000);
}

export interface Chapter {
  id: string;
  label: string;
  caption: string;
  run: (s: Script) => Promise<void>;
}

/** The pane that tails the log, once the splits chapter has opened it. */
const logPane = (s: Script) => {
  const state = s.get();
  return Object.values(state.panes).find((pane) =>
    pane.lines.some((line) => line.k === "cmd" && line.text.startsWith("tail")),
  )?.id;
};

export const CHAPTERS: Chapter[] = [
  {
    id: "shell",
    label: "Shell",
    caption: "Real shell sessions on a native PTY. Your prompt, your tools, your keys.",
    async run(s) {
      s.d({ type: "selectWorkspace", workspace: 1 });
      await s.wait(900);
      await type(s, 1, "cargo test -p neptune-model -q");
      await s.wait(260);
      s.d({ type: "commit", pane: 1 });
      await s.wait(700);
      s.d({ type: "print", pane: 1, lines: [...CARGO_TEST.slice(0, 2), out("")] });
      // One dot per passing test, as they finish.
      for (let done = 3; done <= TEST_COUNT; done += 3) {
        s.d({ type: "amend", pane: 1, line: out(".".repeat(done)) });
        await s.wait(55);
      }
      await s.wait(180);
      s.d({ type: "print", pane: 1, lines: CARGO_TEST.slice(3) });
      s.d({ type: "ready", pane: 1 });
      await s.wait(1700);
    },
  },
  {
    id: "tabs",
    label: "Tabs",
    caption: "Terminals share a place as tabs. A finished job rings its tab, the sidebar and the bell.",
    async run(s) {
      await key(s, "T", "New tab");
      s.d({ type: "newTab", pane: 1 });
      const build = active(s);
      await s.wait(700);
      await type(s, build, "cargo build --release; printf '\\e]9;Build finished\\a'");
      await s.wait(260);
      s.d({ type: "commit", pane: build });
      s.d({ type: "print", pane: build, lines: [BUILDING] });
      await s.wait(900);
      // The build keeps running while its tab is out of view.
      await key(s, "PgUp", "Previous tab");
      s.d({ type: "focus", pane: 1 });
      await s.wait(1500);
      s.d({ type: "print", pane: build, lines: [BUILT] });
      s.d({ type: "notify", pane: build, title: "", body: "Build finished", at: Date.now() });
      s.d({ type: "ready", pane: build });
      await s.wait(2200);
      // Looking at the terminal acknowledges its alert.
      await key(s, "PgDn", "Next tab");
      s.d({ type: "focus", pane: build });
      await s.wait(1300);
      s.d({ type: "focus", pane: 1 });
      await s.wait(700);
    },
  },
  {
    id: "splits",
    label: "Splits",
    caption:
      "Split right or below, then carry a terminal by its tab to rearrange. Each pane is its own shell.",
    async run(s) {
      let second: number | null = null;
      if (s.compact) {
        await key(s, "E", "Split below");
        s.d({ type: "split", pane: 1, axis: "horizontal" });
      } else {
        await key(s, "D", "Split right");
        s.d({ type: "split", pane: 1, axis: "vertical" });
        second = active(s);
        await s.wait(650);
        await type(s, second, "git log --oneline -5");
        await enter(s, second, GIT_LOG, { gap: 45 });
        await s.wait(800);
        await key(s, "E", "Split below");
        s.d({ type: "split", pane: second, axis: "horizontal" });
      }
      const tail = active(s);
      await s.wait(650);
      await type(s, tail, "tail -f ~/code/api/api.log");
      await enter(s, tail, LOG, { gap: 230, running: true });
      await s.wait(700);
      if (second !== null) {
        // The shell keeps running while its terminal is carried.
        await carry(s, second, { kind: "beside", pane: 1, edge: "bottom" });
        s.d({ type: "focus", pane: tail });
        await s.wait(400);
      }
      // The gutter between panes is also the split handle.
      const root = activeWorkspace(s.get())!.layout;
      if (root.kind === "split") {
        s.d({ type: "ratio", split: root.id, ratio: second === null ? 0.44 : 0.56 });
      }
      await s.wait(1100);
      await key(s, "Enter", "Zoom terminal");
      s.d({ type: "zoom" });
      await s.wait(1500);
      await key(s, "Enter", "Show all terminals");
      s.d({ type: "zoom" });
      await s.wait(1100);
    },
  },
  {
    id: "search",
    label: "Search",
    caption: "Find in scrollback from the toolbar. Nothing covers or resizes the shell.",
    async run(s) {
      const pane = logPane(s);
      if (pane === undefined) return;
      s.d({ type: "focus", pane });
      await s.wait(300);
      await key(s, "F", "Find in terminal");
      s.d({ type: "search", open: true, query: "" });
      await s.wait(500);
      for (const query of ["W", "WA", "WAR", "WARN"]) {
        s.d({ type: "search", open: true, query });
        await s.wait(120);
      }
      await s.wait(900);
      // Output keeps arriving; new matches light up as they land.
      for (const line of LOG_LATER) {
        s.d({ type: "print", pane, lines: [line] });
        await s.wait(700);
      }
      await s.wait(900);
      s.d({ type: "search", open: false });
      await s.wait(600);
    },
  },
  {
    id: "commands",
    label: "Commands",
    caption: "Every action, with its shortcut, in one palette.",
    async run(s) {
      await key(s, "P", "Command palette");
      s.d({ type: "overlay", overlay: { kind: "palette", query: "", selected: 0, typed: false } });
      await s.wait(1100);
      for (let selected = 1; selected <= 3; selected++) {
        s.d({ type: "overlay", overlay: { kind: "palette", query: "", selected, typed: false } });
        await s.wait(240);
      }
      await s.wait(500);
      for (const query of ["s", "ss", "ssh"]) {
        s.d({ type: "overlay", overlay: { kind: "palette", query, selected: 0, typed: false } });
        await s.wait(150);
      }
      await s.wait(1000);
      s.d({ type: "overlay", overlay: { kind: "ssh", workspace: null, host: "", typed: false } });
      await s.wait(500);
    },
  },
  {
    id: "ssh",
    label: "SSH",
    caption: "Point a workspace at a host. Every terminal in it opens there, with your own SSH config and keys.",
    async run(s) {
      const destination = "deploy@staging";
      for (let index = 1; index <= destination.length; index++) {
        s.d({
          type: "overlay",
          overlay: { kind: "ssh", workspace: null, host: destination.slice(0, index), typed: false },
        });
        await s.wait(55 + ((index * 29) % 40));
      }
      await s.wait(700);
      s.d({ type: "connect", workspace: null, destination });
      const first = active(s);
      await s.wait(1200);
      s.d({ type: "ready", pane: first });
      await s.wait(500);
      await type(s, first, "hostname");
      await enter(s, first, [out("staging")]);
      await s.wait(700);
      // New splits open on the same host.
      await key(s, s.compact ? "E" : "D", s.compact ? "Split below" : "Split right");
      s.d({ type: "split", pane: first, axis: s.compact ? "horizontal" : "vertical" });
      const second = active(s);
      await s.wait(1100);
      s.d({ type: "ready", pane: second });
      await s.wait(1500);
    },
  },
  {
    id: "workspaces",
    label: "Workspaces",
    caption: "One workspace per project, gathered in folders. Layouts come back on launch; shells start fresh.",
    async run(s) {
      s.d({ type: "selectWorkspace", workspace: 2 });
      await s.wait(1400);
      s.d({ type: "selectWorkspace", workspace: 3 });
      await s.wait(1100);
      if (!s.compact) {
        // Folders gather workspaces; collapsing one never suspends its shells.
        s.d({ type: "collapseGroup", group: 1, collapsed: true });
        await s.wait(1300);
        s.d({ type: "collapseGroup", group: 1, collapsed: false });
        await s.wait(1000);
        await key(s, "B", "Toggle sidebar");
        s.d({ type: "toggleSidebar" });
        await s.wait(1300);
        await key(s, "B", "Toggle sidebar");
        s.d({ type: "toggleSidebar" });
        await s.wait(800);
      }
      s.d({ type: "selectWorkspace", workspace: 1 });
      await s.wait(1900);
    },
  },
];

class Cancelled extends Error {}

export interface TourEvents {
  /** A chapter begins; `duration` is how long it will play, in milliseconds. */
  onChapter: (index: number, duration: number) => void;
}

/**
 * Plays chapters in a loop. It can start from any chapter by replaying the
 * earlier ones at once, pause while off screen, and stop when a visitor takes
 * the window over.
 */
export class Tour {
  private token = { cancelled: true };
  private paused = false;
  private resume: (() => void) | null = null;

  constructor(
    private store: Store,
    private events: TourEvents,
    private compact: () => boolean,
  ) {}

  get playing() {
    return !this.token.cancelled;
  }

  stop() {
    this.token.cancelled = true;
    this.resume?.();
  }

  setPaused(paused: boolean) {
    this.paused = paused;
    if (!paused) this.resume?.();
  }

  /** Shows a chapter's end state without playing it. */
  async show(index: number) {
    this.stop();
    await this.replay(this.store, index + 1);
  }

  play(from = 0) {
    this.stop();
    const token = { cancelled: false };
    this.token = token;
    void this.loop(from, token);
  }

  private script(
    store: Store,
    wait: (ms: number) => Promise<void>,
    compact: boolean,
  ) {
    return { d: store.dispatch, get: store.get, wait, compact };
  }

  /** Brings `store` to the state in which chapter `count` begins. */
  private async replay(store: Store, count: number, compact = this.compact()) {
    store.dispatch({ type: "load", state: BASE });
    const instant = this.script(store, async () => {}, compact);
    for (let index = 0; index < count; index++) await CHAPTERS[index].run(instant);
    // The shortcut display belongs to the moment it was pressed.
    store.dispatch({ type: "load", state: { ...store.get(), hud: null, drag: null } });
  }

  private async measure(index: number, compact: boolean): Promise<number> {
    const scratch = new Store(BASE);
    await this.replay(scratch, index, compact);
    let total = 0;
    await CHAPTERS[index].run(
      this.script(
        scratch,
        async (ms) => {
          total += ms;
        },
        compact,
      ),
    );
    return total;
  }

  private async loop(from: number, token: { cancelled: boolean }) {
    const wait = async (ms: number) => {
      await new Promise((resolve) => setTimeout(resolve, ms));
      while (this.paused && !token.cancelled) {
        await new Promise<void>((resolve) => {
          this.resume = resolve;
        });
      }
      if (token.cancelled) throw new Cancelled();
    };
    try {
      const compact = this.compact();
      await this.replay(this.store, from, compact);
      for (let index = from; ; index = (index + 1) % CHAPTERS.length) {
        if (token.cancelled) return;
        if (index === 0) this.store.dispatch({ type: "load", state: BASE });
        this.events.onChapter(index, await this.measure(index, compact));
        await CHAPTERS[index].run(this.script(this.store, wait, compact));
      }
    } catch (error) {
      if (!(error instanceof Cancelled)) throw error;
    }
  }
}
