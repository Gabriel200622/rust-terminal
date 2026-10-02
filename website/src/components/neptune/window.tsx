"use client";

// Neptune's window, rebuilt for the browser from the app's own layout code
// (`src/ui/chrome.rs`, `src/ui/workspace.rs`) and theme tokens. Sizes are the
// app's logical points; the window responds to its own width as the app does:
// the sidebar yields below 820 points and the command field below 760.

import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { Icon } from "../icons";
import { usePrefs, useMac } from "../prefs";
import { Button, IconButton, Keycaps, shortcut } from "./controls";
import {
  activeWorkspace,
  arrange,
  dropArea,
  length,
  panesOf,
  validDestination,
  type Action,
  type Box,
  type Destination,
  type Divider,
  type Edge,
  type Pane,
  type State,
  type Store,
  type Workspace,
} from "./model";
import { Palette, commands, matches, type Command } from "./palette";
import { PreferencesBody } from "./preferences";
import { execute } from "./shell";
import { Terminal } from "./terminal";

type Dispatch = (action: Action) => void;

const boxStyle = (box: Box): React.CSSProperties => ({
  left: length(box.x),
  top: length(box.y),
  width: length(box.w),
  height: length(box.h),
});

// Workspace tiles take a stable identity colour from their id.
const IDENTITY = [
  "--blue",
  "--purple",
  "--emerald",
  "--orange",
  "--pink",
  "--indigo",
  "--amber",
  "--crimson",
];
const identity = (id: number) => IDENTITY[(Math.max(1, id) - 1) % IDENTITY.length];

const SLIDE = "duration-[160ms] ease-out";

function WindowControls() {
  return (
    <div className="group/lights absolute top-4 left-4 z-30 flex gap-2">
      {(
        [
          ["Close window", "#ff5f57", "M-2.4 -2.4L2.4 2.4M2.4 -2.4L-2.4 2.4"],
          ["Minimize", "#febc2e", "M-3 0H3"],
          ["Maximize", "#28c840", "M-3 0H3M0 -3V3"],
        ] as const
      ).map(([label, color, glyph]) => (
        <span
          key={label}
          title={label}
          className="grid size-3 place-items-center rounded-full shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.19)]"
          style={{ background: color }}
        >
          <svg
            viewBox="-6 -6 12 12"
            className="size-3 opacity-0 transition-opacity duration-100 group-hover/lights:opacity-100"
            stroke={`color-mix(in srgb, black 62%, ${color})`}
            strokeWidth="1.2"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d={glyph} />
          </svg>
        </span>
      ))}
    </div>
  );
}

function WorkspaceRow({
  workspace,
  index,
  state,
  dispatch,
}: {
  workspace: Workspace;
  index: number;
  state: State;
  dispatch: Dispatch;
}) {
  const selected = workspace.id === state.active;
  const ids = panesOf(workspace.layout);
  const running = ids.some((id) => state.panes[id]?.status !== "exited");
  const color = identity(workspace.id);
  const menu = state.overlay.kind === "menu" && state.overlay.workspace === workspace.id;
  // A carried terminal can be dropped on any workspace but its own, as long
  // as that workspace is on the same machine.
  const accepts = !selected && workspace.remote === activeWorkspace(state)?.remote;
  const receiving =
    state.drag?.to?.kind === "workspace" && state.drag.to.workspace === workspace.id;
  return (
    <div
      data-workspace={workspace.id}
      data-accepts={accepts}
      className={`group/row absolute inset-x-0 h-[46px] rounded-control transition-[top,background-color] ${SLIDE} ${
        selected ? "bg-pressed" : menu ? "bg-hover" : "hover:bg-hover"
      }`}
      style={{ top: index * 48 }}
    >
      <span
        className={`pointer-events-none absolute inset-0 rounded-control bg-accent/20 shadow-[inset_0_0_0_1.5px_color-mix(in_srgb,var(--accent)_90%,transparent)] transition-opacity duration-[120ms] ease-out ${
          receiving ? "opacity-100" : "opacity-0"
        }`}
      />
      <button
        type="button"
        aria-current={selected}
        onClick={() => dispatch({ type: "selectWorkspace", workspace: workspace.id })}
        className="absolute inset-0 flex cursor-pointer items-center rounded-control pr-[30px] pl-[9px] text-left"
      >
        <span
          className="relative grid size-7 shrink-0 place-items-center rounded-lg text-[12.5px] font-semibold"
          style={
            selected
              ? {
                  background: `var(${color})`,
                  color: color === "--amber" ? "var(--on-amber)" : "#fff",
                }
              : {
                  background: `color-mix(in srgb, var(${color}) var(--tile-alpha), transparent)`,
                  color: `color-mix(in srgb, black var(--tile-ink-mix), var(${color}))`,
                }
          }
        >
          {(workspace.name.match(/[\p{L}\p{N}]/u)?.[0] ?? "·").toUpperCase()}
          {!running && (
            // Every terminal here has stopped.
            <span className="absolute -top-[3.5px] -right-[3.5px] size-[9px] rounded-full bg-danger ring-[1.5px] ring-chrome" />
          )}
        </span>
        <span className="ml-2.5 min-w-0 flex-1">
          <span
            className={`block truncate text-[13px] leading-[17px] font-medium ${
              selected ? "text-fg" : "text-[color-mix(in_srgb,var(--fg)_45%,var(--secondary))] group-hover/row:text-fg"
            }`}
          >
            {workspace.name}
          </span>
          <span className="flex items-center gap-1 truncate text-[11px] leading-[15px] text-muted">
            {workspace.remote ? (
              <>
                <Icon name="globe" size={11} />
                <span className="truncate">{workspace.remote}</span>
              </>
            ) : (
              <span className="truncate">{workspace.cwd}</span>
            )}
          </span>
        </span>
      </button>
      {ids.length > 1 && (
        <span
          className={`pointer-events-none absolute top-1/2 right-[5px] grid size-6 -translate-y-1/2 place-items-center text-[11px] font-medium text-muted group-hover/row:opacity-0 ${
            menu ? "opacity-0" : ""
          }`}
        >
          {ids.length}
        </span>
      )}
      <button
        type="button"
        aria-label={`Actions for ${workspace.name}`}
        aria-expanded={menu}
        onClick={() =>
          dispatch({
            type: "overlay",
            overlay: menu ? { kind: "none" } : { kind: "menu", workspace: workspace.id },
          })
        }
        className={`absolute top-1/2 right-[5px] grid size-6 -translate-y-1/2 cursor-pointer place-items-center rounded-[6px] text-secondary hover:bg-pressed hover:text-fg focus-visible:opacity-100 group-hover/row:opacity-100 ${
          menu ? "bg-pressed opacity-100" : "opacity-0"
        }`}
      >
        <Icon name="ellipsis" size={14} />
      </button>
    </div>
  );
}

/** A workspace's menu. The hovered row takes the accent, as native menus do. */
function WorkspaceMenu({
  workspace,
  index,
  count,
  dispatch,
}: {
  workspace: Workspace;
  index: number;
  count: number;
  dispatch: Dispatch;
}) {
  const item = (
    icon: Parameters<typeof Icon>[0]["name"],
    label: string,
    action: Action,
    destructive = false,
  ) => (
    <button
      type="button"
      role="menuitem"
      onClick={() => {
        dispatch({ type: "overlay", overlay: { kind: "none" } });
        dispatch(action);
      }}
      className={`flex h-7 w-full cursor-pointer items-center gap-[10px] rounded-[6px] pr-2.5 pl-2 text-left text-[13px] ${
        destructive
          ? "text-danger hover:bg-danger hover:text-white"
          : "text-fg hover:bg-accent hover:text-on-accent"
      }`}
    >
      <Icon name={icon} size={14} />
      {label}
    </button>
  );
  return (
    <div
      role="menu"
      aria-label={`Actions for ${workspace.name}`}
      className="absolute left-[150px] z-40 w-[210px] animate-fade-in rounded-pane border border-edge bg-elevated p-[5px] shadow-popup"
      style={{ top: Math.min(74 + index * 48 + 36, 420) }}
    >
      {index > 0 &&
        item("arrowUp", "Move up", {
          type: "moveWorkspace",
          workspace: workspace.id,
          index: index - 1,
        })}
      {index + 1 < count &&
        item("arrowDown", "Move down", {
          type: "moveWorkspace",
          workspace: workspace.id,
          index: index + 1,
        })}
      {workspace.remote
        ? item("globe", "Disconnect from SSH", { type: "disconnect", workspace: workspace.id })
        : item("globe", "Connect over SSH…", {
            type: "overlay",
            overlay: { kind: "ssh", workspace: workspace.id, host: "", typed: true },
          })}
      <div className="mx-2 my-1 h-px bg-separator" />
      {item("close", "Close workspace", { type: "closeWorkspace", workspace: workspace.id }, true)}
    </div>
  );
}

function Sidebar({ state, dispatch, mac }: { state: State; dispatch: Dispatch; mac: boolean }) {
  return (
    <aside
      aria-label="Workspaces"
      className={`absolute inset-y-0 left-0 hidden w-[216px] -translate-x-full transition-transform @min-[820px]/win:block ${SLIDE} @min-[820px]/win:group-data-[sidebar=open]/win:translate-x-0`}
    >
      <p className="absolute top-[49px] left-[18px] text-[11.5px] font-medium text-muted">
        Workspaces
      </p>
      <div className="absolute inset-x-2 top-[74px] bottom-[54px] overflow-hidden">
        {state.workspaces.map((workspace, index) => (
          <WorkspaceRow
            key={workspace.id}
            workspace={workspace}
            index={index}
            state={state}
            dispatch={dispatch}
          />
        ))}
      </div>
      <div className="absolute inset-x-2 bottom-2 flex h-[34px] items-center">
        <button
          type="button"
          title={`New workspace   ${shortcut(mac, "T")}`}
          onClick={() => dispatch({ type: "newWorkspace" })}
          className="flex h-[34px] min-w-0 flex-1 cursor-pointer items-center gap-[9px] rounded-control pl-2 text-[12.5px] font-medium text-secondary hover:bg-hover hover:text-fg active:bg-pressed"
        >
          <Icon name="plus" size={14} />
          <span className="truncate">New workspace</span>
        </button>
        <IconButton
          icon="settings"
          label="Preferences"
          hint={mac ? "⌘," : "Ctrl+,"}
          className="ml-1"
          onClick={() => dispatch({ type: "overlay", overlay: { kind: "settings" } })}
        />
      </div>
    </aside>
  );
}

function SearchField({
  state,
  dispatch,
  className,
}: {
  state: State;
  dispatch: Dispatch;
  className: string;
}) {
  const input = useRef<HTMLInputElement>(null);
  const { query, typed } = state.search;
  useEffect(() => {
    if (typed) input.current?.focus({ preventScroll: true });
  }, [typed]);
  const close = () => dispatch({ type: "search", open: false });
  return (
    <div
      role="search"
      className={`relative flex h-[30px] items-center rounded-control bg-control pr-px pl-[10px] shadow-[inset_0_0_0_1px_var(--accent),0_0_0_3px_color-mix(in_srgb,var(--accent)_28%,transparent)] ${className}`}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          close();
        }
        event.stopPropagation();
      }}
    >
      <Icon name="search" size={13} className="text-muted" />
      {typed ? (
        <input
          ref={input}
          value={query}
          onChange={(event) =>
            dispatch({ type: "search", open: true, query: event.target.value, typed: true })
          }
          aria-label="Terminal search"
          placeholder="Find in scrollback"
          spellCheck={false}
          autoComplete="off"
          className="ml-[7px] h-full min-w-0 flex-1 bg-transparent text-[12.5px] text-fg caret-accent outline-none select-text placeholder:text-muted"
        />
      ) : (
        <span className="ml-[7px] min-w-0 flex-1 truncate text-[12.5px] text-fg">
          {query || <span className="text-muted">Find in scrollback</span>}
          <span className="ml-px inline-block h-[1.15em] w-[1.5px] bg-accent align-[-0.2em]" />
        </span>
      )}
      <IconButton icon="arrowUp" label="Previous match" hint="Shift+Enter" />
      <IconButton icon="arrowDown" label="Next match" hint="Enter" />
      <IconButton icon="close" label="Close search" hint="Esc" onClick={close} />
    </div>
  );
}

function Toolbar({ state, dispatch, mac }: { state: State; dispatch: Dispatch; mac: boolean }) {
  const workspace = activeWorkspace(state);
  const pane = workspace ? state.panes[workspace.active] : undefined;
  const searching = state.search.open && !!pane;
  const location = pane?.remote ?? pane?.cwd;
  return (
    <div className="@container/bar absolute inset-x-0 top-0 h-11">
      <div
        className={`flex h-full items-center pr-2 pl-[84px] transition-[padding] @min-[820px]/win:pl-[124px] ${SLIDE} @min-[820px]/win:group-data-[sidebar=open]/win:pl-[14px]`}
      >
        {/* Leading: the workspace, then the focused terminal. */}
        <div
          className={`flex min-w-0 items-baseline gap-2.5 ${
            searching ? "hidden flex-1 basis-0 @min-[760px]/bar:flex" : "flex-1 basis-0"
          }`}
        >
          {workspace && (
            <>
              <span className="truncate text-[13px] font-medium text-fg">{workspace.name}</span>
              {pane && (
                <span className="min-w-[48px] flex-1 truncate text-[12px] text-secondary">
                  {pane.title} — {location}
                </span>
              )}
            </>
          )}
        </div>

        {/* Centre: commands, or search while searching. */}
        {searching ? (
          <SearchField
            state={state}
            dispatch={dispatch}
            className="mx-3.5 min-w-0 flex-1 @min-[760px]/bar:w-[clamp(300px,42cqw,440px)] @min-[760px]/bar:flex-none"
          />
        ) : (
          <button
            type="button"
            aria-label="Command palette"
            onClick={() =>
              dispatch({
                type: "overlay",
                overlay: { kind: "palette", query: "", selected: 0, typed: true },
              })
            }
            className="mx-3.5 hidden h-7 w-[clamp(210px,32cqw,320px)] shrink-0 cursor-pointer items-center rounded-control bg-control pr-1.5 pl-[10px] text-muted hover:bg-pressed @min-[760px]/bar:flex"
          >
            <Icon name="search" size={13} />
            <span className="ml-[7px] min-w-0 flex-1 truncate text-left text-[12.5px]">
              Search commands
            </span>
            <Keycaps chord={shortcut(mac, "P")} />
          </button>
        )}

        {/* Trailing: controls for the focused terminal. */}
        <div
          className={`flex items-center justify-end gap-0.5 ${
            searching ? "flex-none @min-[760px]/bar:flex-1 @min-[760px]/bar:basis-0" : "flex-1 basis-0"
          }`}
        >
          {state.zoomed && (
            <button
              type="button"
              title={`Show all terminals   ${shortcut(mac, "Enter")}`}
              onClick={() => dispatch({ type: "zoom" })}
              className="mr-1.5 flex h-[22px] shrink-0 animate-fade-in cursor-pointer items-center gap-[5px] rounded-full bg-accent/20 pr-2.5 pl-[9px] text-[11.5px] font-medium text-accent hover:bg-accent/30"
            >
              <Icon name="minimize" size={12} />
              Zoomed
            </button>
          )}
          <IconButton
            icon="command"
            label="Command palette"
            hint={shortcut(mac, "P")}
            className={searching ? "hidden" : "@min-[760px]/bar:hidden"}
            onClick={() =>
              dispatch({
                type: "overlay",
                overlay: { kind: "palette", query: "", selected: 0, typed: true },
              })
            }
          />
          {pane && (
            <>
              <IconButton
                icon="search"
                label="Find in terminal"
                hint={shortcut(mac, "F")}
                onClick={() => dispatch({ type: "search", open: true, typed: true })}
              />
              <IconButton
                icon="splitVertical"
                label="Split right"
                hint={shortcut(mac, "D")}
                onClick={() => dispatch({ type: "split", pane: pane.id, axis: "vertical" })}
              />
              <IconButton
                icon="splitHorizontal"
                label="Split below"
                hint={shortcut(mac, "E")}
                onClick={() => dispatch({ type: "split", pane: pane.id, axis: "horizontal" })}
              />
            </>
          )}
          {/* "New workspace" moves here while the sidebar is away. */}
          <IconButton
            icon="plus"
            label="New workspace"
            hint={shortcut(mac, "T")}
            className={`@min-[820px]/win:group-data-[sidebar=open]/win:hidden`}
            onClick={() => dispatch({ type: "newWorkspace" })}
          />
        </div>
      </div>
    </div>
  );
}

/** A floating status line with one action, near the pane's bottom edge. */
function StatusCapsule({ pane, dispatch }: { pane: Pane; dispatch: Dispatch }) {
  return (
    <div className="absolute bottom-[17px] left-1/2 flex h-[34px] max-w-[calc(100%-16px)] -translate-x-1/2 animate-fade-in items-center rounded-full border border-edge bg-elevated pr-[5px] pl-[13px] shadow-popup">
      <span className="size-[7px] shrink-0 rounded-full bg-muted" />
      <span className="mr-3 ml-[9px] truncate text-[12px] font-medium text-fg">
        Process exited
      </span>
      <button
        type="button"
        title={pane.remote ? "Connect again   Enter" : "Start a new shell   Enter"}
        onClick={() => dispatch({ type: "restart", pane: pane.id })}
        className="h-6 shrink-0 cursor-pointer rounded-full bg-accent/16 px-2.5 text-[12px] font-medium text-accent hover:bg-accent/26 active:bg-accent/34"
      >
        {pane.remote ? "Reconnect" : "Restart"}
      </button>
    </div>
  );
}

function PaneView({
  pane,
  box,
  selected,
  multiple,
  search,
  gliding,
  live,
  lifted,
  mac,
  dispatch,
  onCarry,
}: {
  pane: Pane;
  box: Box;
  selected: boolean;
  /** More than one pane is visible, so panes carry headers and focus cues. */
  multiple: boolean;
  search: string;
  gliding: boolean;
  /** The window holds the keyboard, so the focused pane's cursor is solid. */
  live: boolean;
  /** The terminal is being carried elsewhere, so its pane recedes. */
  lifted: boolean;
  mac: boolean;
  dispatch: Dispatch;
  /** The header title is dragged: the pointer carries the terminal. */
  onCarry: (pane: number, clientX: number, clientY: number) => void;
}) {
  const press = useRef<{ x: number; y: number; carrying: boolean } | null>(null);
  const focus = () => dispatch({ type: "focus", pane: pane.id });
  const control = (
    icon: Parameters<typeof Icon>[0]["name"],
    label: string,
    key: string,
    actions: Action[],
    className = "",
  ) => (
    <IconButton
      icon={icon}
      label={label}
      hint={shortcut(mac, key)}
      className={className}
      onClick={() => actions.forEach(dispatch)}
    />
  );
  const split = (axis: "vertical" | "horizontal"): Action[] => [
    { type: "focus", pane: pane.id },
    { type: "split", pane: pane.id, axis },
  ];
  return (
    <section
      aria-label={`Terminal pane ${pane.id}`}
      data-pane={pane.id}
      onPointerDown={focus}
      className={`group/pane @container/pane absolute animate-pane-in overflow-hidden rounded-pane bg-bg ${
        gliding ? `transition-[left,top,width,height] ${SLIDE}` : ""
      }`}
      style={boxStyle(box)}
    >
      {multiple && (
        <header className="absolute inset-x-0 top-0 flex h-[30px] items-center pr-1 pl-3.5">
          {/* The title is also the handle that carries the terminal elsewhere. */}
          <div
            onDoubleClick={() => dispatch({ type: "zoom" })}
            onPointerDown={(event) => {
              if (event.button !== 0) return;
              event.currentTarget.setPointerCapture(event.pointerId);
              press.current = { x: event.clientX, y: event.clientY, carrying: false };
            }}
            onPointerMove={(event) => {
              const held = press.current;
              if (!held) return;
              const moved = Math.hypot(event.clientX - held.x, event.clientY - held.y);
              if (!held.carrying && moved < 5) return;
              held.carrying = true;
              onCarry(pane.id, event.clientX, event.clientY);
            }}
            onPointerUp={() => {
              if (press.current?.carrying) dispatch({ type: "release" });
              press.current = null;
            }}
            onPointerCancel={() => {
              if (press.current?.carrying) dispatch({ type: "drag", drag: null });
              press.current = null;
            }}
            className="flex h-full min-w-0 flex-1 cursor-grab items-baseline gap-[9px] pt-[7px]"
          >
            <span
              className={`truncate text-[12px] font-medium ${selected ? "text-fg" : "text-secondary"}`}
            >
              {pane.title}
            </span>
            <span className="min-w-0 truncate text-[11.5px] text-muted">
              {pane.remote ?? pane.cwd.split("/").pop()}
            </span>
          </div>
          <div
            className={`flex shrink-0 transition-opacity duration-[120ms] ease-out group-hover/pane:opacity-100 ${
              selected ? "opacity-100" : "opacity-0"
            }`}
          >
            {control("maximize", "Zoom terminal", "Enter", [{ type: "focus", pane: pane.id }, { type: "zoom" }], "hidden @min-[260px]/pane:block")}
            {control("splitVertical", "Split right", "D", split("vertical"), "hidden @min-[260px]/pane:block")}
            {control("splitHorizontal", "Split below", "E", split("horizontal"), "hidden @min-[260px]/pane:block")}
            {control("close", "Close terminal", "W", [{ type: "closePane", pane: pane.id }])}
          </div>
        </header>
      )}
      <div
        data-focused={selected && live}
        className={`absolute inset-x-3 bottom-2.5 cursor-text ${multiple ? "top-[30px]" : "top-2.5"}`}
      >
        <Terminal pane={pane} search={selected ? search : ""} />
      </div>
      {/* Unfocused panes recede slightly; the focused one carries the accent. */}
      <div
        className={`pointer-events-none absolute inset-0 bg-bg transition-opacity duration-[140ms] ease-out ${
          multiple && !selected ? "opacity-[0.24]" : "opacity-0"
        }`}
      />
      <div className="pointer-events-none absolute inset-0 rounded-pane shadow-[inset_0_0_0_1px_var(--separator)]" />
      <div
        className={`pointer-events-none absolute inset-0 rounded-pane shadow-[inset_0_0_0_1.5px_color-mix(in_srgb,var(--accent)_85%,transparent)] transition-opacity duration-[140ms] ease-out ${
          multiple && selected ? "opacity-100" : "opacity-0"
        }`}
      />
      <div
        className={`pointer-events-none absolute inset-0 rounded-pane bg-chrome transition-opacity duration-[120ms] ease-out ${
          lifted ? "opacity-60" : "opacity-0"
        }`}
      />
      {pane.status === "starting" && (
        <p className="absolute inset-0 flex items-center justify-center gap-2 text-[12.5px] text-muted">
          <svg viewBox="0 0 16 16" className="spinner size-3.5" fill="none" aria-hidden="true">
            <path d="M8 1.5a6.5 6.5 0 106.5 6.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
          {pane.remote ? "Connecting…" : "Starting shell…"}
        </p>
      )}
      {pane.status === "exited" && <StatusCapsule pane={pane} dispatch={dispatch} />}
    </section>
  );
}

/** The gutter between panes is also the split handle. */
function DividerHandle({
  divider,
  stage,
  dispatch,
  onDrag,
}: {
  divider: Divider;
  stage: React.RefObject<HTMLDivElement | null>;
  dispatch: Dispatch;
  onDrag: (dragging: boolean) => void;
}) {
  const [dragging, setDragging] = useState(false);
  const vertical = divider.axis === "vertical";
  const move = (event: React.PointerEvent) => {
    const element = stage.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    const scale = rect.width / element.offsetWidth || 1;
    const start = vertical ? divider.area.x : divider.area.y;
    const length = vertical ? divider.area.w : divider.area.h;
    const size = vertical ? rect.width : rect.height;
    const origin = vertical ? rect.left : rect.top;
    const from = (start.pct / 100) * size + start.px * scale;
    const span = (length.pct / 100) * size + length.px * scale;
    const pointer = vertical ? event.clientX : event.clientY;
    dispatch({ type: "ratio", split: divider.id, ratio: (pointer - origin - from) / span });
  };
  return (
    <div
      role="separator"
      aria-orientation={vertical ? "vertical" : "horizontal"}
      aria-label="Resize split"
      className={`group/divider absolute z-10 touch-none ${
        vertical ? "-mx-0.5 cursor-col-resize px-0.5" : "-my-0.5 cursor-row-resize py-0.5"
      }`}
      style={{ ...boxStyle(divider), boxSizing: "content-box" }}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
        onDrag(true);
      }}
      onPointerMove={(event) => dragging && move(event)}
      onPointerUp={() => {
        setDragging(false);
        onDrag(false);
      }}
      onPointerCancel={() => {
        setDragging(false);
        onDrag(false);
      }}
      onDoubleClick={() => dispatch({ type: "ratio", split: divider.id, ratio: 0.5 })}
    >
      <span
        className={`absolute top-1/2 left-1/2 -translate-1/2 rounded-[2px] transition-opacity duration-[120ms] ease-out group-hover/divider:opacity-100 ${
          vertical ? "h-10 max-h-[calc(100%-16px)] w-[3px]" : "h-[3px] w-10 max-w-[calc(100%-16px)]"
        } ${dragging ? "bg-accent opacity-100" : "bg-muted opacity-0"}`}
      />
    </div>
  );
}

function Stage({
  state,
  dispatch,
  mac,
  onGrab,
  live,
}: {
  state: State;
  dispatch: Dispatch;
  mac: boolean;
  onGrab: () => void;
  live: boolean;
}) {
  const stage = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const workspace = activeWorkspace(state);
  const className = `absolute top-11 right-1.5 bottom-1.5 left-1.5 transition-[left] ${SLIDE} @min-[820px]/win:group-data-[sidebar=open]/win:left-0`;
  if (!workspace) {
    return (
      <div className={className}>
        {/* A calm placeholder for a window without a workspace. */}
        <div className="grid size-full place-items-center rounded-pane bg-bg shadow-[inset_0_0_0_1px_var(--separator)]">
          <div className="flex flex-col items-center text-center">
            <span className="grid size-[52px] place-items-center rounded-sheet bg-accent/16 text-accent">
              <Icon name="terminal" size={24} />
            </span>
            <p className="mt-5 text-[15px] font-semibold text-fg">No open workspaces</p>
            <p className="mt-1 text-[12.5px] text-secondary">
              Start a shell in your home directory.
            </p>
            <Button kind="primary" className="mt-5" onClick={() => dispatch({ type: "newWorkspace" })}>
              New workspace
            </Button>
          </div>
        </div>
      </div>
    );
  }
  const arranged = arrange(workspace.layout);
  const several = arranged.panes.size > 1;
  const visible = state.zoomed
    ? [[workspace.active, arrange({ kind: "pane", pane: workspace.active }).panes.get(workspace.active)!] as const]
    : [...arranged.panes];
  const multiple = several && !state.zoomed;
  const drag = state.drag;
  const target =
    drag?.to && drag.to.kind !== "workspace" ? arranged.panes.get(drag.to.pane) : undefined;

  // Where the carried terminal would land: against the nearest edge of the
  // pane under the pointer, in that pane's place when near its centre, or in
  // a workspace of the sidebar.
  const carry = (pane: number, clientX: number, clientY: number) => {
    const element = stage.current;
    if (!element) return;
    if (!drag) onGrab();
    const inside = (rect: DOMRect) =>
      clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom;
    let to: Destination | null = null;
    for (const node of element.querySelectorAll<HTMLElement>("[data-pane]")) {
      const rect = node.getBoundingClientRect();
      const id = Number(node.dataset.pane);
      if (id === pane || !inside(rect)) continue;
      const x = (clientX - rect.left) / Math.max(1, rect.width);
      const y = (clientY - rect.top) / Math.max(1, rect.height);
      const edges: [number, Edge][] = [
        [x, "left"],
        [1 - x, "right"],
        [y, "top"],
        [1 - y, "bottom"],
      ];
      const [distance, edge] = edges.reduce((near, next) => (next[0] < near[0] ? next : near));
      to = distance > 0.3 ? { kind: "swap", pane: id } : { kind: "beside", pane: id, edge };
    }
    const rows = element
      .closest("[role=application]")
      ?.querySelectorAll<HTMLElement>("[data-workspace][data-accepts=true]");
    for (const row of rows ?? []) {
      if (inside(row.getBoundingClientRect())) {
        to = { kind: "workspace", workspace: Number(row.dataset.workspace) };
      }
    }
    const rect = element.getBoundingClientRect();
    const scale = rect.width / element.offsetWidth || 1;
    dispatch({
      type: "drag",
      drag: {
        pane,
        x: `${(clientX - rect.left) / scale}px`,
        y: `${(clientY - rect.top) / scale}px`,
        to,
      },
    });
  };

  return (
    <div ref={stage} className={className}>
      {visible.map(([id, box]) => {
        const pane = state.panes[id];
        return (
          pane && (
            <PaneView
              key={id}
              pane={pane}
              box={box}
              selected={id === workspace.active}
              multiple={multiple}
              search={state.search.open ? state.search.query : ""}
              gliding={!dragging}
              live={live}
              lifted={drag?.pane === id}
              mac={mac}
              dispatch={dispatch}
              onCarry={carry}
            />
          )
        );
      })}
      {multiple &&
        arranged.dividers.map((divider) => (
          <DividerHandle
            key={divider.id}
            divider={divider}
            stage={stage}
            dispatch={dispatch}
            onDrag={(active) => {
              if (active) onGrab();
              setDragging(active);
            }}
          />
        ))}
      {/* The area a drop would take. It glides between areas as the pointer moves. */}
      {drag?.to && target && (
        <div
          className="pointer-events-none absolute z-20 animate-fade-in rounded-pane bg-accent/20 shadow-[inset_0_0_0_1.5px_color-mix(in_srgb,var(--accent)_90%,transparent)] transition-[left,top,width,height] duration-[120ms] ease-out"
          style={boxStyle(dropArea(target, drag.to))}
        >
          {drag.to.kind === "swap" && (
            <span className="absolute top-1/2 left-1/2 grid size-8 -translate-1/2 place-items-center rounded-full border border-edge bg-elevated text-accent shadow-popup">
              <Icon name="swap" size={14} />
            </span>
          )}
        </div>
      )}
      {/* A chip with the terminal's name follows the pointer. */}
      {drag && (
        <div
          className={`pointer-events-none absolute z-30 flex h-7 animate-fade-in items-center gap-[7px] rounded-full border border-edge bg-elevated pr-3 pl-[9px] text-[12px] font-medium whitespace-nowrap text-fg shadow-popup ${
            drag.scripted ? "transition-[left,top] duration-[600ms] ease-out" : ""
          }`}
          style={{ left: `calc(${drag.x} + 14px)`, top: `calc(${drag.y} + 16px)` }}
        >
          <Icon name="terminal" size={13} className="text-secondary" />
          {state.panes[drag.pane]?.title}
        </div>
      )}
    </div>
  );
}

function Sheet({
  title,
  width,
  children,
}: {
  title: string;
  width: number;
  children: React.ReactNode;
}) {
  return (
    <div
      role="dialog"
      aria-label={title}
      className="absolute top-1/2 left-1/2 flex max-h-[calc(100%-24px)] -translate-1/2 flex-col"
      style={{ width: `min(${width}px, calc(100% - 24px))` }}
    >
      <div className="flex min-h-0 animate-sheet-in flex-col overflow-hidden rounded-sheet border border-edge bg-elevated shadow-sheet">
        {children}
      </div>
    </div>
  );
}

function SshSheet({
  overlay,
  dispatch,
}: {
  overlay: Extract<State["overlay"], { kind: "ssh" }>;
  dispatch: Dispatch;
}) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (overlay.typed) input.current?.focus({ preventScroll: true });
  }, [overlay.typed]);
  const close = () => dispatch({ type: "overlay", overlay: { kind: "none" } });
  const problem = overlay.host.trim() !== "" && !validDestination(overlay.host);
  const connect = () =>
    dispatch({ type: "connect", workspace: overlay.workspace, destination: overlay.host });
  const title = overlay.workspace === null ? "New SSH workspace" : "Connect over SSH";
  return (
    <Sheet title={title} width={420}>
      <div
        onKeyDown={(event) => {
          if (event.key === "Enter") connect();
          else if (event.key === "Escape") close();
          event.stopPropagation();
        }}
      >
        <h3 className="flex h-[52px] items-center px-5 text-[15px] font-semibold text-fg">
          {title}
        </h3>
        <div className="px-5">
          <div
            className={`flex h-[30px] items-center rounded-control bg-control px-2.5 ${
              overlay.typed
                ? "shadow-[inset_0_0_0_1px_var(--separator)] focus-within:shadow-[inset_0_0_0_1px_var(--accent),0_0_0_3px_color-mix(in_srgb,var(--accent)_28%,transparent)]"
                : "shadow-[inset_0_0_0_1px_var(--accent),0_0_0_3px_color-mix(in_srgb,var(--accent)_28%,transparent)]"
            }`}
          >
            {overlay.typed ? (
              <input
                ref={input}
                value={overlay.host}
                onChange={(event) =>
                  dispatch({ type: "overlay", overlay: { ...overlay, host: event.target.value } })
                }
                aria-label="SSH host"
                placeholder="user@host"
                spellCheck={false}
                autoCapitalize="none"
                autoComplete="off"
                className="h-full min-w-0 flex-1 bg-transparent text-[13px] text-fg caret-accent outline-none select-text placeholder:text-muted"
              />
            ) : (
              <span className="truncate text-[13px] text-fg">
                {overlay.host || <span className="text-muted">user@host</span>}
                <span className="ml-px inline-block h-[1.15em] w-[1.5px] bg-accent align-[-0.2em]" />
              </span>
            )}
          </div>
          <p className={`mt-3 text-[12px] leading-[1.45] ${problem ? "text-danger" : "text-secondary"}`}>
            {problem
              ? "SSH host must be a destination such as user@host, without spaces or a leading dash"
              : overlay.workspace === null
                ? "Terminals open on the host using your SSH configuration and keys. Sign-in prompts appear in the terminal."
                : "Every terminal in this workspace restarts on the host. Running processes will stop."}
          </p>
        </div>
        <div className="mt-1 flex h-[62px] items-center justify-end gap-2 px-5">
          <Button onClick={close}>Cancel</Button>
          <Button kind="primary" onClick={connect}>
            Connect
          </Button>
        </div>
      </div>
    </Sheet>
  );
}

function SettingsSheet({ dispatch }: { dispatch: Dispatch }) {
  const { reset } = usePrefs();
  const close = () => dispatch({ type: "overlay", overlay: { kind: "none" } });
  return (
    <Sheet title="Preferences" width={500}>
      <div className="flex h-[52px] shrink-0 items-center justify-between pr-3 pl-5">
        <h3 className="text-[15px] font-semibold text-fg">Preferences</h3>
        <IconButton icon="close" label="Close preferences" onClick={close} />
      </div>
      <div className="quiet-scroll min-h-0 overflow-y-auto">
        <PreferencesBody />
      </div>
      <div className="flex h-[60px] shrink-0 items-center justify-between border-t border-separator px-4">
        <Button kind="quiet" onClick={reset}>
          Reset to defaults
        </Button>
        <Button kind="primary" onClick={close}>
          Done
        </Button>
      </div>
    </Sheet>
  );
}

/** The shortcut just performed, shown the way a screen recording would. */
function Hud({ hud, mac }: { hud: NonNullable<State["hud"]>; mac: boolean }) {
  return (
    <div className="hud pointer-events-none absolute bottom-6 left-1/2 z-50 -translate-x-1/2">
      <div
        key={hud.n}
        className="flex h-[38px] animate-hud-in items-center gap-2.5 rounded-full border border-edge bg-elevated pr-4 pl-2.5 text-[12.5px] font-medium whitespace-nowrap text-fg shadow-popup"
      >
        <Keycaps chord={shortcut(mac, hud.keys)} large className="text-secondary" />
        {hud.label}
      </div>
    </div>
  );
}

export function NeptuneWindow({
  store,
  onTakeover,
  touring,
}: {
  store: Store;
  /** A visitor used the window, so a scripted tour should let go of it. */
  onTakeover: () => void;
  /** A scripted tour is typing, as a focused window would be typed in. */
  touring: boolean;
}) {
  const state = useSyncExternalStore(store.subscribe, store.get, store.get);
  const dispatch = store.dispatch;
  const { prefs, set: setPrefs } = usePrefs();
  const mac = useMac();
  const root = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState(false);
  const overlay = state.overlay;
  const workspace = activeWorkspace(state);

  // A terminal that is starting becomes ready shortly after.
  const starting = Object.values(state.panes)
    .filter((pane) => pane.status === "starting")
    .map((pane) => pane.id)
    .join(",");
  useEffect(() => {
    if (!starting) return;
    const timer = window.setTimeout(() => {
      for (const id of starting.split(",")) {
        if (store.get().panes[Number(id)]?.status === "starting") {
          store.dispatch({ type: "ready", pane: Number(id) });
        }
      }
    }, 1300);
    return () => window.clearTimeout(timer);
  }, [starting, store]);

  const list: Command[] =
    overlay.kind === "palette"
      ? commands(state, prefs, mac, dispatch, setPrefs).filter((command) =>
          matches(command, overlay.query),
        )
      : [];

  const perform = (key: string, label: string, ...actions: Action[]) => {
    dispatch({ type: "hud", keys: key, label });
    actions.forEach(dispatch);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    const target = event.target as HTMLElement;
    const pane = workspace ? state.panes[workspace.active] : undefined;
    const chord = mac ? event.metaKey : event.ctrlKey && event.shiftKey;
    if (chord && !event.altKey) {
      const key = event.key.length === 1 ? event.key.toUpperCase() : event.key;
      const open: Action = {
        type: "overlay",
        overlay: { kind: "palette", query: "", selected: 0, typed: true },
      };
      const bound: Record<string, () => void> = {
        P: () => perform("P", "Command palette", open),
        B: () => perform("B", "Toggle sidebar", { type: "toggleSidebar" }),
        ...(pane && {
          D: () => perform("D", "Split right", { type: "split", pane: pane.id, axis: "vertical" }),
          E: () => perform("E", "Split below", { type: "split", pane: pane.id, axis: "horizontal" }),
          F: () => perform("F", "Find in terminal", { type: "search", open: true, typed: true }),
          Enter: () =>
            perform("Enter", state.zoomed ? "Show all terminals" : "Zoom terminal", { type: "zoom" }),
        }),
      };
      if (bound[key]) {
        event.preventDefault();
        bound[key]();
        return;
      }
    }
    if (state.drag) {
      // Escape puts a carried terminal back; the shell does not receive it.
      if (event.key === "Escape") dispatch({ type: "drag", drag: null });
      return;
    }
    if (overlay.kind !== "none") {
      if (event.key === "Escape") dispatch({ type: "overlay", overlay: { kind: "none" } });
      return;
    }
    // A focused control keeps its own activation keys.
    if (target.closest("button, input") && (event.key === "Enter" || event.key === " ")) return;
    if (event.key === "Tab" || event.metaKey || event.altKey || !pane) return;
    if (event.key === "Escape") {
      root.current?.blur();
      return;
    }
    if (pane.status === "exited") {
      if (event.key === "Enter") dispatch({ type: "restart", pane: pane.id });
      return;
    }
    if (pane.status !== "running") return;
    if (event.ctrlKey) {
      const key = event.key.toLowerCase();
      if (key === "c") {
        event.preventDefault();
        if (pane.prompt) {
          dispatch({ type: "input", pane: pane.id, input: `${pane.input}^C` });
          dispatch({ type: "commit", pane: pane.id });
        } else {
          dispatch({ type: "print", pane: pane.id, lines: [{ k: "out", spans: [{ t: "^C" }] }] });
        }
        dispatch({ type: "ready", pane: pane.id, ok: false });
      } else if (key === "l") {
        event.preventDefault();
        dispatch({ type: "clear", pane: pane.id });
      }
      return;
    }
    if (!pane.prompt) return;
    if (event.key === "Enter") {
      event.preventDefault();
      execute({ dispatch, setPrefs }, pane);
    } else if (event.key === "Backspace") {
      event.preventDefault();
      dispatch({ type: "input", pane: pane.id, input: pane.input.slice(0, -1) });
    } else if (event.key.length === 1) {
      event.preventDefault();
      dispatch({ type: "input", pane: pane.id, input: pane.input + event.key });
    }
  };

  return (
    <div
      ref={root}
      role="application"
      aria-label="Neptune, an interactive demonstration"
      aria-roledescription="terminal window"
      tabIndex={0}
      data-sidebar={state.sidebar ? "open" : "closed"}
      className="group/win relative size-full overflow-hidden rounded-window bg-chrome text-[13px] leading-normal shadow-window outline-none select-none [container:win_/_size]"
      onFocus={() => setFocused(true)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setFocused(false);
      }}
      onKeyDownCapture={onTakeover}
      onClickCapture={onTakeover}
      onKeyDown={onKeyDown}
      onClick={(event) => {
        // The terminal holds the keyboard, so a pressed control hands it back.
        const target = event.target as HTMLElement;
        if (overlay.kind === "none" && !target.closest("input")) {
          root.current?.focus({ preventScroll: true });
        }
      }}
    >
      <Sidebar state={state} dispatch={dispatch} mac={mac} />
      <div className={`absolute inset-y-0 right-0 left-0 transition-[left] ${SLIDE} @min-[820px]/win:group-data-[sidebar=open]/win:left-[216px]`}>
        <Toolbar state={state} dispatch={dispatch} mac={mac} />
        <Stage
          state={state}
          dispatch={dispatch}
          mac={mac}
          onGrab={onTakeover}
          live={touring || focused}
        />
      </div>
      <WindowControls />
      <IconButton
        icon="sidebar"
        label="Toggle sidebar"
        hint={shortcut(mac, "B")}
        className={`absolute top-2 left-[80px] z-30 hidden transition-[left] @min-[820px]/win:block ${SLIDE} @min-[820px]/win:group-data-[sidebar=open]/win:left-[180px]`}
        onClick={() => dispatch({ type: "toggleSidebar" })}
      />

      {overlay.kind === "menu" && (
        <>
          <div
            className="absolute inset-0 z-30"
            onClick={() => dispatch({ type: "overlay", overlay: { kind: "none" } })}
          />
          {state.workspaces.map(
            (item, index) =>
              item.id === overlay.workspace && (
                <WorkspaceMenu
                  key={item.id}
                  workspace={item}
                  index={index}
                  count={state.workspaces.length}
                  dispatch={dispatch}
                />
              ),
          )}
        </>
      )}

      {(overlay.kind === "palette" || overlay.kind === "ssh" || overlay.kind === "settings") && (
        <div className="absolute inset-0 z-40">
          {/* The dim follows the window's rounded shape. */}
          <div
            className="absolute inset-0 animate-fade-in rounded-window bg-scrim"
            onClick={() => dispatch({ type: "overlay", overlay: { kind: "none" } })}
          />
          {overlay.kind === "palette" && (
            <Palette
              list={list}
              query={overlay.query}
              selected={overlay.selected}
              typed={overlay.typed}
              onQuery={(query) =>
                dispatch({ type: "overlay", overlay: { ...overlay, query, selected: 0 } })
              }
              onSelect={(selected) =>
                dispatch({ type: "overlay", overlay: { ...overlay, selected } })
              }
              onRun={(command) => {
                // Close first: the command itself may open another overlay.
                dispatch({ type: "overlay", overlay: { kind: "none" } });
                command.run();
                root.current?.focus({ preventScroll: true });
              }}
              onClose={() => dispatch({ type: "overlay", overlay: { kind: "none" } })}
            />
          )}
          {overlay.kind === "ssh" && <SshSheet overlay={overlay} dispatch={dispatch} />}
          {overlay.kind === "settings" && <SettingsSheet dispatch={dispatch} />}
        </div>
      )}

      {state.hud && <Hud hud={state.hud} mac={mac} />}
    </div>
  );
}
