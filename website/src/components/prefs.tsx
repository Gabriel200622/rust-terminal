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

// The settings of `config.example.toml` that change how Pace looks.
export const THEMES = ["graphite", "dusk", "light"] as const;
export type Theme = (typeof THEMES)[number];

export const ACCENTS = [
  "blue",
  "indigo",
  "purple",
  "pink",
  "red",
  "orange",
  "yellow",
  "green",
  "graphite",
] as const;
export type Accent = (typeof ACCENTS)[number];

/** The CSS variable carrying each accent, resolved per theme. */
export const ACCENT_VAR: Record<Accent, string> = {
  blue: "--blue",
  indigo: "--indigo",
  purple: "--purple",
  pink: "--pink",
  red: "--crimson",
  orange: "--orange",
  yellow: "--amber",
  green: "--emerald",
  graphite: "--slate",
};

export const CURSORS = ["block", "beam", "underline"] as const;
export type Cursor = (typeof CURSORS)[number];

export interface Prefs {
  theme: Theme;
  accent: Accent;
  fontSize: number;
  lineHeight: number;
  cursor: Cursor;
  blink: boolean;
}

export const DEFAULT_PREFS: Prefs = {
  theme: "graphite",
  accent: "blue",
  fontSize: 14,
  lineHeight: 1.4,
  cursor: "block",
  blink: false,
};

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

  // Preferences apply as they change, as in the app. The document carries
  // them, so terminals restyle without re-rendering.
  useEffect(() => {
    const root = document.documentElement;
    const previous = applied.current;
    applied.current = prefs;
    if (previous.theme !== prefs.theme || previous.accent !== prefs.accent) {
      root.classList.add("theme-shift");
      const timer = window.setTimeout(
        () => root.classList.remove("theme-shift"),
        260,
      );
      root.dataset.theme = prefs.theme;
      root.dataset.accent = prefs.accent;
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
  const reset = useCallback(() => setPrefs(DEFAULT_PREFS), []);
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
