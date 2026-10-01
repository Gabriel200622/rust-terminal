"use client";

import { PreferencesBody } from "../pace/preferences";
import { usePrefs } from "../prefs";
import { Button } from "../pace/controls";

function Setting({
  name,
  value,
  tone,
}: {
  name: string;
  value: string;
  tone: number;
}) {
  return (
    <div className="term-row">
      {name}
      <span className="text-muted"> = </span>
      {/* A changed value fades in, so the edit is easy to spot. */}
      <span key={value} className="animate-fade-in" style={{ color: `var(--ansi-${tone})` }}>
        {value}
      </span>
    </div>
  );
}

/** The saved settings, as `config.toml` holds them. */
function ConfigPane() {
  const { prefs } = usePrefs();
  const prompt = (
    <div className="term-row">
      <span style={{ color: "var(--ansi-4)", fontWeight: 700 }}>~/.config/pace</span>
    </div>
  );
  const chevron = (
    <svg
      viewBox="0 0 10 16"
      className="inline-block h-[0.95em] w-[0.6em] align-[-0.1em]"
      fill="none"
      stroke="var(--ansi-2)"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M2.5 3.5L7.5 8l-5 4.5" />
    </svg>
  );
  return (
    <div className="rounded-window bg-chrome p-1.5 shadow-window">
      <div
        data-focused="true"
        className="min-h-[264px] overflow-hidden rounded-pane bg-bg px-3 py-2.5 shadow-[inset_0_0_0_1px_var(--separator)]"
      >
        <div className="term text-fg" aria-label="config.toml" role="img">
          {prompt}
          <div className="term-row">{chevron} cat config.toml</div>
          <Setting name="theme" value={`"${prefs.theme}"`} tone={2} />
          <Setting name="accent" value={`"${prefs.accent}"`} tone={2} />
          <Setting name="font_size" value={prefs.fontSize.toFixed(1)} tone={3} />
          <Setting name="line_height" value={String(prefs.lineHeight)} tone={3} />
          <Setting name="cursor" value={`"${prefs.cursor}"`} tone={2} />
          <Setting name="cursor_blink" value={String(prefs.blink)} tone={5} />
          {prompt}
          <div className="term-row">
            {chevron} <span className="cursor" />
          </div>
        </div>
      </div>
    </div>
  );
}

export function Customize() {
  const { reset } = usePrefs();
  return (
    <section
      id="customize"
      className="mx-auto grid w-full max-w-[1180px] items-center gap-x-16 gap-y-10 px-5 sm:px-8 lg:grid-cols-[minmax(0,1fr)_minmax(0,500px)]"
    >
      <div className="min-w-0">
        <h2 className="text-[clamp(30px,4.4vw,44px)] leading-[1.05] font-semibold tracking-[-0.035em] text-balance">
          Make it yours.
        </h2>
        <p className="mt-4 max-w-[26rem] text-[16px] text-secondary">
          Three themes, nine accents, and type set the way you read. Try them: this
          whole page follows.
        </p>
        <div className="mt-8 max-w-[30rem]">
          <ConfigPane />
        </div>
      </div>

      <div
        role="group"
        aria-label="Preferences"
        className="w-full max-w-[500px] justify-self-center overflow-hidden rounded-sheet border border-edge bg-elevated shadow-sheet select-none lg:justify-self-end"
      >
        <p className="flex h-[52px] items-center px-5 text-[15px] font-semibold text-fg">
          Preferences
        </p>
        <PreferencesBody />
        <div className="flex h-[60px] items-center border-t border-separator px-4">
          <Button kind="quiet" onClick={reset}>
            Reset to defaults
          </Button>
        </div>
      </div>
    </section>
  );
}
