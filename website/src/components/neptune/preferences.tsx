"use client";

// Preferences: grouped settings that apply as they change. The layout follows
// `src/ui/preferences.rs`; the same sheet opens inside the demo window and
// stands on its own further down the page.

import {
  ACCENTS,
  ACCENT_VAR,
  THEMES,
  usePrefs,
  type Accent,
  type Theme,
} from "../prefs";
import { Segmented, Slider, Stepper, Toggle } from "./controls";

const THEME_NAMES: Record<Theme, string> = {
  graphite: "Graphite",
  dusk: "Dusk",
  light: "Light",
};

const capitalized = (text: string) => text[0].toUpperCase() + text.slice(1);

/** A miniature of the window in a theme's own materials. */
function ThemeTile({
  theme,
  selected,
  onSelect,
}: {
  theme: Theme;
  selected: boolean;
  onSelect: () => void;
}) {
  const bars: [number, string][] = [
    [0.42, "var(--accent)"],
    [0.74, "var(--fg)"],
    [0.56, "var(--ansi-2)"],
    [0.66, "var(--secondary)"],
    [0.3, "var(--ansi-5)"],
  ];
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      className="group/tile flex min-w-0 flex-1 cursor-pointer flex-col items-center gap-[7px] pb-1 outline-none"
    >
      <span
        className={`relative block w-full rounded-[11px] p-[3px] ring-accent/50 group-focus-visible/tile:ring-[1.5px] ${
          selected ? "shadow-[inset_0_0_0_2px_var(--accent)]" : ""
        }`}
      >
        {/* The preview is drawn in its own theme, whatever the page wears. */}
        <span
          data-theme={theme}
          className="relative block h-[70px] overflow-hidden rounded-lg bg-chrome"
        >
          {["#ff5f57", "#febc2e", "#28c840"].map((light, index) => (
            <span
              key={light}
              className="absolute top-1.5 size-1 rounded-full"
              style={{ left: 6 + index * 6, background: light }}
            />
          ))}
          {["var(--pressed)", "var(--hover)", "var(--hover)"].map((fill, index) => (
            <span
              key={index}
              className="absolute left-1.5 h-[7px] w-[calc(27%-10px)] rounded-[3px]"
              style={{ top: 20 + index * 11, background: fill }}
            />
          ))}
          <span className="absolute top-1.5 right-[5px] bottom-[5px] left-[27%] rounded-[5px] bg-bg">
            {bars.map(([length, fill], index) => (
              <span
                key={index}
                className="absolute left-[7px] h-[3.5px] rounded-full opacity-85"
                style={{
                  top: 8 + index * 9,
                  width: `calc((100% - 14px) * ${length})`,
                  background: fill,
                }}
              />
            ))}
          </span>
        </span>
        {/* The outline belongs to the surrounding sheet, not the preview. */}
        {!selected && (
          <span className="absolute inset-[3px] rounded-lg shadow-[inset_0_0_0_1px_var(--border)] group-hover/tile:shadow-[inset_0_0_0_1px_var(--muted)]" />
        )}
      </span>
      <span
        className={`text-[12px] font-medium ${selected ? "text-fg" : "text-secondary"}`}
      >
        {THEME_NAMES[theme]}
      </span>
    </button>
  );
}

function AccentDots({
  current,
  onSelect,
}: {
  current: Accent;
  onSelect: (accent: Accent) => void;
}) {
  return (
    <div role="radiogroup" aria-label="Accent" className="flex items-center gap-[7px]">
      {ACCENTS.map((accent) => (
        <button
          key={accent}
          type="button"
          role="radio"
          aria-checked={accent === current}
          aria-label={`${capitalized(accent)} accent`}
          title={capitalized(accent)}
          onClick={() => onSelect(accent)}
          className="grid size-5 shrink-0 cursor-pointer place-items-center"
        >
          <span
            className="grid size-4 place-items-center rounded-full shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.16)] transition-transform duration-100 hover:scale-[1.07]"
            style={{ background: `var(${ACCENT_VAR[accent]})` }}
          >
            {accent === current && <span className="size-1.5 rounded-full bg-white" />}
          </span>
        </button>
      ))}
    </div>
  );
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <p className="flex h-6 items-center px-1 text-[11.5px] font-medium text-secondary">
      {children}
    </p>
  );
}

/** An inset card of setting rows separated by hairlines. */
function Group({ children }: { children: React.ReactNode }) {
  return (
    <div className="rounded-pane bg-control">{children}</div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="relative flex h-[42px] items-center justify-between gap-3 pr-3 pl-3.5 not-first:before:absolute not-first:before:top-0 not-first:before:right-0 not-first:before:left-3.5 not-first:before:h-px not-first:before:bg-separator">
      <span className="shrink-0 text-[13px] text-fg">{label}</span>
      <span className="flex min-w-0 items-center justify-end gap-3">{children}</span>
    </div>
  );
}

/** The appearance and text settings of the Preferences sheet. */
export function PreferencesBody() {
  const { prefs, set } = usePrefs();
  return (
    <div className="px-5 pb-[18px] text-fg">
      <Label>Appearance</Label>
      <div role="radiogroup" aria-label="Theme" className="mt-0.5 flex gap-2.5">
        {THEMES.map((theme) => (
          <ThemeTile
            key={theme}
            theme={theme}
            selected={prefs.theme === theme}
            onSelect={() => set({ theme })}
          />
        ))}
      </div>
      <div className="mt-2">
        <Group>
          <Row label="Accent">
            <AccentDots current={prefs.accent} onSelect={(accent) => set({ accent })} />
          </Row>
        </Group>
      </div>

      <div className="mt-3.5">
        <Label>Text</Label>
        <Group>
          <Row label="Font size">
            <Stepper
              label="Font size"
              value={prefs.fontSize}
              min={9}
              max={32}
              step={1}
              text={`${prefs.fontSize} pt`}
              onChange={(fontSize) => set({ fontSize })}
            />
          </Row>
          <Row label="Line spacing">
            <Slider
              label="Line spacing"
              value={prefs.lineHeight}
              min={1}
              max={2}
              step={0.05}
              onChange={(lineHeight) => set({ lineHeight })}
            />
            <span className="w-[34px] shrink-0 text-right text-[12.5px] font-medium text-secondary tabular-nums">
              {prefs.lineHeight.toFixed(2)}
            </span>
          </Row>
        </Group>
      </div>

      <div className="mt-3.5">
        <Label>Cursor</Label>
        <Group>
          <Row label="Style">
            <Segmented
              label="Cursor style"
              value={prefs.cursor}
              options={[
                ["block", "Block"],
                ["beam", "Beam"],
                ["underline", "Underline"],
              ]}
              onChange={(cursor) => set({ cursor })}
            />
          </Row>
          <Row label="Blink">
            <Toggle
              on={prefs.blink}
              label="Blink cursor"
              onChange={(blink) => set({ blink })}
            />
          </Row>
        </Group>
      </div>
    </div>
  );
}
