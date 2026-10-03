"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { ACCENTS, BUILTINS, isDark, themeVariables, type Accent, type Theme } from "./neptune/themes";

export { ACCENTS, type Accent, type Theme };

export const CURSORS = ["block", "beam", "underline"] as const;
export type Cursor = (typeof CURSORS)[number];

/** The settings of `config.example.toml` that Preferences changes. */
export interface Prefs {
  theme: Theme;
  /** Theme ids starred in the catalog, in the order they were starred. */
  favorites: Theme[];
  /** For Graphite, Dusk and Light only; a palette brings its own accent. */
  accent: Accent;
  windowZoom: number;
  fontSize: number;
  lineHeight: number;
  scrollback: number;
  /** A shell program; absent for the platform default. */
  shell: string | null;
  cursor: Cursor;
  blink: boolean;
  restoreWorkspaces: boolean;
  confirmClose: boolean;
  warnProcesses: boolean;
  checkUpdates: boolean;
  releaseChannel: "stable" | "beta";
  desktopNotifications: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  theme: BUILTINS[0],
  favorites: [],
  accent: "blue",
  windowZoom: 1,
  fontSize: 14,
  lineHeight: 1.4,
  scrollback: 10000,
  shell: null,
  cursor: "block",
  blink: false,
  restoreWorkspaces: true,
  confirmClose: true,
  warnProcesses: true,
  checkUpdates: true,
  releaseChannel: "stable",
  desktopNotifications: true,
};

export const WINDOW_ZOOM = { min: 0.2, max: 5 };

interface PrefsContext {
  prefs: Prefs;
  set: (patch: Partial<Prefs>) => void;
  reset: () => void;
}

const Context = createContext<PrefsContext | null>(null);

export function usePrefs(): PrefsContext {
  const context = useContext(Context);
  if (!context) throw new Error("usePrefs needs a PrefsProvider");
  return context;
}

export function PrefsProvider({ children }: { children: React.ReactNode }) {
  const [prefs, setPrefs] = useState(DEFAULT_PREFS);
  const applied = useRef(DEFAULT_PREFS);
  const painted = useRef<string[]>([]);

  // Preferences apply as they change, as in the app. The document carries
  // them, so terminals restyle without re-rendering.
  useEffect(() => {
    const root = document.documentElement;
    const previous = applied.current;
    applied.current = prefs;
    if (previous.theme.id !== prefs.theme.id || previous.accent !== prefs.accent) {
      root.classList.add("theme-shift");
      const timer = window.setTimeout(
        () => root.classList.remove("theme-shift"),
        260,
      );
      // An imported palette lays its colours over the base theme it resembles.
      const variables = themeVariables(prefs.theme);
      root.dataset.theme = prefs.theme.colors
        ? isDark(prefs.theme.colors)
          ? "graphite"
          : "light"
        : prefs.theme.id;
      root.dataset.accent = prefs.accent;
      for (const name of painted.current) root.style.removeProperty(name);
      painted.current = Object.keys(variables);
      for (const [name, value] of Object.entries(variables)) root.style.setProperty(name, value);
      const meta = document.querySelector('meta[name="theme-color"]');
      meta?.setAttribute(
        "content",
        getComputedStyle(root).getPropertyValue("--bg").trim(),
      );
      return () => {
        window.clearTimeout(timer);
        root.classList.remove("theme-shift");
      };
    }
  }, [prefs]);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.cursor = prefs.cursor;
    root.dataset.blink = String(prefs.blink);
    root.style.setProperty("--term-size", `${prefs.fontSize}px`);
    root.style.setProperty("--term-leading", String(prefs.lineHeight));
  }, [prefs.cursor, prefs.blink, prefs.fontSize, prefs.lineHeight]);

  const set = useCallback(
    (patch: Partial<Prefs>) => setPrefs((current) => ({ ...current, ...patch })),
    [],
  );
  // Favorites are kept, as the app keeps them and the custom themes.
  const reset = useCallback(
    () => setPrefs((current) => ({ ...DEFAULT_PREFS, favorites: current.favorites })),
    [],
  );
  const value = useMemo(() => ({ prefs, set, reset }), [prefs, set, reset]);
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

const noop = () => () => {};

/** Command on macOS, Ctrl+Shift elsewhere. The server renders the latter. */
export function useMac(): boolean {
  return useSyncExternalStore(
    noop,
    () => /Mac|iPhone|iPad/.test(navigator.platform),
    () => false,
  );
}

/** The user asked for less motion. */
export function useReducedMotion(): boolean {
  return useSyncExternalStore(
    (notify) => {
      const query = window.matchMedia("(prefers-reduced-motion: reduce)");
      query.addEventListener("change", notify);
      return () => query.removeEventListener("change", notify);
    },
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    () => false,
  );
}
