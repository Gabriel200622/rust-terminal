"use client";

// Shared controls, following `src/ui/controls.rs`: every one paints from the
// palette tokens and keeps the app's sizes.

import { useEffect, useRef } from "react";
import { Icon, type IconName } from "../icons";

/** The platform's primary command chord for a key: `Ctrl+Shift+D` or `⌘D`. */
export const shortcut = (mac: boolean, key: string) =>
  mac ? `⌘${key}` : `Ctrl+Shift+${key}`;

/** The platform's editing chord, used for preferences. */
export const editShortcut = (mac: boolean, key: string) =>
  mac ? `⌘${key}` : `Ctrl+${key}`;

/** Splits a chord into its keys. A trailing `+` is the plus key itself. */
function chordKeys(chord: string): string[] {
  const plus = chord.endsWith("+");
  const modifiers = plus ? chord.slice(0, -1).replace(/\+$/, "") : chord;
  return [...modifiers.split("+").filter(Boolean), ...(plus ? ["+"] : [])];
}

export function Keycaps({
  chord,
  className = "",
  large = false,
}: {
  chord: string;
  className?: string;
  large?: boolean;
}) {
  if (!chord) return null;
  return (
    <span className={`inline-flex shrink-0 items-center gap-[3px] ${className}`}>
      {chordKeys(chord).map((key, index) => (
        <kbd
          key={index}
          className={`inline-flex items-center justify-center rounded-cap bg-[color-mix(in_srgb,currentColor_12%,transparent)] font-sans font-medium ${
            large
              ? "h-[22px] min-w-6 px-[7px] text-[11.5px]"
              : "h-[18px] min-w-5 px-[5px] text-[10.5px]"
          }`}
        >
          {key}
        </kbd>
      ))}
    </span>
  );
}

export function IconButton({
  icon,
  label,
  hint,
  onClick,
  className = "",
  size = 16,
  pressed,
}: {
  icon: IconName;
  label: string;
  hint?: string;
  onClick?: () => void;
  className?: string;
  size?: number;
  pressed?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={pressed}
      title={hint ? `${label}   ${hint}` : label}
      onClick={onClick}
      className={`group/icon size-7 shrink-0 cursor-pointer p-px text-secondary outline-none hover:text-fg ${className}`}
    >
      <span className="grid size-full place-items-center rounded-[7px] transition-colors duration-100 group-hover/icon:bg-hover group-focus-visible/icon:ring-[1.5px] group-focus-visible/icon:ring-accent group-active/icon:bg-pressed">
        <Icon name={icon} size={size} />
      </span>
    </button>
  );
}

const BUTTON_KINDS = {
  primary: "bg-accent text-on-accent hover:brightness-110 active:brightness-90",
  secondary: "bg-control text-fg hover:bg-pressed",
  destructive: "bg-danger text-on-danger hover:brightness-110 active:brightness-90",
  quiet: "px-2.5 text-secondary hover:bg-hover active:bg-pressed",
} as const;

export function Button({
  kind = "secondary",
  children,
  onClick,
  className = "",
  disabled = false,
}: {
  kind?: keyof typeof BUTTON_KINDS;
  children: React.ReactNode;
  onClick?: () => void;
  className?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`inline-flex h-[30px] min-w-[72px] shrink-0 cursor-pointer items-center justify-center rounded-control px-4 text-[13px] font-medium whitespace-nowrap transition-[background-color,filter] duration-100 disabled:pointer-events-none disabled:opacity-45 ${BUTTON_KINDS[kind]} ${className}`}
    >
      {children}
    </button>
  );
}

/** A switch for a setting that applies immediately. */
export function Toggle({
  on,
  label,
  onChange,
}: {
  on: boolean;
  label: string;
  onChange: (on: boolean) => void;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => onChange(!on)}
      className={`relative h-[22px] w-[38px] shrink-0 cursor-pointer rounded-full transition-colors duration-150 ${
        on ? "bg-accent" : "bg-pressed"
      }`}
    >
      <span
        className="absolute top-0.5 left-0.5 size-[18px] rounded-full bg-white shadow-[0_1px_0_rgb(0_0_0/0.16)] transition-transform duration-150 ease-out"
        style={{ transform: on ? "translateX(16px)" : undefined }}
      />
    </button>
  );
}

/** Mutually exclusive choices with a thumb that slides to the selection. */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
  width = 228,
}: {
  label: string;
  value: T;
  options: readonly (readonly [T, string])[];
  onChange: (value: T) => void;
  width?: number | string;
}) {
  const selected = Math.max(
    0,
    options.findIndex(([option]) => option === value),
  );
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="relative flex h-7 max-w-full shrink rounded-control bg-control p-0.5"
      style={{ width }}
    >
      <span
        className="absolute top-0.5 bottom-0.5 left-0.5 rounded-[6px] bg-[var(--thumb)] shadow-[0_1px_0_var(--thumb-shadow)] transition-transform duration-150 ease-out"
        style={{
          width: `calc((100% - 4px) / ${options.length})`,
          transform: `translateX(${selected * 100}%)`,
        }}
      />
      {options.map(([option, name]) => (
        <button
          key={option}
          type="button"
          role="radio"
          aria-checked={option === value}
          onClick={() => onChange(option)}
          className={`relative flex-1 cursor-pointer rounded-[6px] text-[12px] font-medium transition-colors duration-100 hover:text-fg ${
            option === value ? "text-fg" : "text-secondary"
          }`}
        >
          {name}
        </button>
      ))}
    </div>
  );
}

/** A continuous value with a filled track. Arrow keys nudge by one step. */
export function Slider({
  label,
  value,
  min,
  max,
  step,
  onChange,
  width = 150,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  width?: number;
}) {
  const track = useRef<HTMLDivElement>(null);
  const snap = (raw: number) =>
    Number(Math.min(max, Math.max(min, Math.round(raw / step) * step)).toFixed(4));
  const fraction = (value - min) / (max - min);
  const from = (clientX: number) => {
    const rect = track.current!.getBoundingClientRect();
    const knob = (8 * rect.width) / track.current!.offsetWidth;
    const travel = Math.max(1, rect.width - knob * 2);
    onChange(snap(min + (max - min) * Math.min(1, Math.max(0, (clientX - rect.left - knob) / travel))));
  };
  return (
    <div
      ref={track}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      className="relative h-[22px] max-w-full shrink cursor-pointer touch-none rounded-full"
      style={{ width }}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        from(event.clientX);
      }}
      onPointerMove={(event) => {
        if (event.currentTarget.hasPointerCapture(event.pointerId)) from(event.clientX);
      }}
      onKeyDown={(event) => {
        const nudge = { ArrowRight: 1, ArrowUp: 1, ArrowLeft: -1, ArrowDown: -1 }[event.key];
        if (!nudge) return;
        event.preventDefault();
        onChange(snap(value + nudge * step));
      }}
    >
      <span className="absolute inset-x-0 top-[9px] h-1 rounded-full bg-pressed" />
      <span
        className="absolute top-[9px] left-0 h-1 rounded-full bg-accent"
        style={{ width: `calc(8px + (100% - 16px) * ${fraction})` }}
      />
      <span
        className="absolute top-[3px] size-4 rounded-full bg-white shadow-[0_1px_0_rgb(0_0_0/0.2)]"
        style={{ left: `calc((100% - 16px) * ${fraction})` }}
      />
    </div>
  );
}

/** A bounded number adjusted in fixed steps. */
export function Stepper({
  label,
  value,
  min,
  max,
  step,
  text,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  text: string;
  onChange: (value: number) => void;
}) {
  const cell = (direction: number, icon: IconName, name: string) => {
    const next = Math.min(max, Math.max(min, value + direction * step));
    const enabled = next !== value;
    return (
      <button
        type="button"
        disabled={!enabled}
        aria-label={`${name} ${label.toLowerCase()}`}
        onClick={() => onChange(next)}
        className="grid h-7 w-[30px] cursor-pointer place-items-center text-secondary enabled:hover:text-fg disabled:cursor-default disabled:text-muted"
      >
        <span className="grid size-[24px] place-items-center rounded-[6px] [button:enabled:active_&]:bg-pressed [button:enabled:hover_&]:bg-hover">
          <Icon name={icon} size={12} />
        </span>
      </button>
    );
  };
  return (
    <div className="flex h-7 w-28 shrink-0 items-center rounded-control bg-control">
      {cell(-1, "minus", "Decrease")}
      <span className="flex-1 text-center text-[12.5px] font-medium text-fg tabular-nums">
        {text}
      </span>
      {cell(1, "plus", "Increase")}
    </div>
  );
}

/** An unread count in the attention colour, tinted to stay quiet in chrome. */
export function Pill({ count, className = "" }: { count: number; className?: string }) {
  return (
    <span
      className={`inline-grid h-[18px] shrink-0 place-items-center rounded-full bg-[color-mix(in_srgb,var(--attention)_var(--pill-alpha),transparent)] text-[10.5px] font-semibold text-[color-mix(in_srgb,black_var(--pill-ink-mix),var(--attention))] tabular-nums ${
        count < 10 ? "w-[18px]" : count < 100 ? "w-6" : "w-[31px]"
      } ${className}`}
    >
      {count > 99 ? "99+" : count}
    </span>
  );
}

/** The surface of an editable field: filled, with an accent ring in focus. */
export const FIELD =
  "rounded-control bg-control shadow-[inset_0_0_0_1px_var(--separator)] focus-within:shadow-[inset_0_0_0_1px_var(--accent),0_0_0_3px_color-mix(in_srgb,var(--accent)_28%,transparent)]";
/** The same surface while a scripted tour types in it. */
export const FIELD_FOCUSED =
  "rounded-control bg-control shadow-[inset_0_0_0_1px_var(--accent),0_0_0_3px_color-mix(in_srgb,var(--accent)_28%,transparent)]";

/** A search field: a magnifier leads the text and a clear control trails it. */
export function SearchInput({
  value,
  onChange,
  placeholder,
  label,
  autoFocus = false,
  className = "",
  onKeyDown,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  label: string;
  /** Takes the keyboard when it appears, without scrolling the page to it. */
  autoFocus?: boolean;
  className?: string;
  onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (autoFocus) input.current?.focus({ preventScroll: true });
  }, [autoFocus]);
  return (
    <div className={`flex h-[30px] min-w-0 items-center pl-[10px] ${FIELD} ${className}`}>
      <Icon name="search" size={13} className="text-muted" />
      <input
        ref={input}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        aria-label={label}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        maxLength={256}
        className="ml-[7px] h-full min-w-0 flex-1 bg-transparent text-[13px] text-fg caret-accent outline-none select-text placeholder:text-muted"
      />
      {value ? (
        <button
          type="button"
          aria-label="Clear search"
          title="Clear search"
          onClick={() => {
            onChange("");
            input.current?.focus({ preventScroll: true });
          }}
          className="mr-[5px] grid size-[22px] shrink-0 cursor-pointer place-items-center rounded-[6px] text-secondary hover:bg-hover hover:text-fg"
        >
          <Icon name="close" size={9} />
        </button>
      ) : (
        <span className="w-[10px] shrink-0" />
      )}
    </div>
  );
}
