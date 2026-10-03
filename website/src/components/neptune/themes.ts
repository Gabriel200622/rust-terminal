// How a theme colours the window, following `Palette` in `src/theme.rs`: the
// three original themes keep their own materials, and an imported palette
// supplies exact terminal colours from which the window's are derived.

export type Rgb = readonly [number, number, number];

/** A theme in use: a Neptune theme, or a palette with its colours. */
export interface Theme {
  /** The config value: `graphite`, `dusk`, `light` or `iterm:<name>`. */
  id: string;
  name: string;
  /** 23 colours as RRGGBB, for an imported palette. */
  colors?: string;
}

export const BUILTINS: readonly Theme[] = [
  { id: "graphite", name: "Graphite" },
  { id: "dusk", name: "Dusk" },
  { id: "light", name: "Light" },
];

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

const ACCENT_COLORS: Record<Accent, [number, number]> = {
  blue: [0x0a84ff, 0x007aff],
  indigo: [0x7d7aff, 0x5856d6],
  purple: [0xbf5af2, 0xaf52de],
  pink: [0xff4f79, 0xff2d55],
  red: [0xff5a52, 0xff3b30],
  orange: [0xff9f0a, 0xf08a00],
  yellow: [0xffd60a, 0xe0a800],
  green: [0x30d158, 0x28a745],
  graphite: [0x98989f, 0x7c7c82],
};

/** Everything a theme decides, as the app's `Palette` holds it. */
export interface Look {
  dark: boolean;
  bg: Rgb;
  chrome: Rgb;
  elevated: Rgb;
  fg: Rgb;
  secondary: Rgb;
  muted: Rgb;
  accent: Rgb;
  onAccent: Rgb;
  green: Rgb;
  yellow: Rgb;
  red: Rgb;
  attention: Rgb;
  terminalFg: Rgb;
  cursor: Rgb;
  /** Absent in the original themes, which tint the cell instead. */
  cursorText?: Rgb;
  selection: Rgb;
  ansi: Rgb[];
}

const WHITE: Rgb = [255, 255, 255];
const BLACK: Rgb = [0, 0, 0];
const INK: Rgb = [0x1d, 0x1d, 0x1f];

const rgb = (value: number): Rgb => [value >> 16, (value >> 8) & 255, value & 255];

export const hex = ([r, g, b]: Rgb) =>
  `#${[r, g, b].map((channel) => channel.toString(16).padStart(2, "0")).join("")}`;

/** Mixes `top` over `base`. */
export function mix(base: Rgb, top: Rgb, amount: number): Rgb {
  const t = Math.min(1, Math.max(0, amount));
  const channel = (a: number, b: number) => Math.round(a + (b - a) * t);
  return [channel(base[0], top[0]), channel(base[1], top[1]), channel(base[2], top[2])];
}

function relativeLuminance(color: Rgb): number {
  const linear = (value: number) => {
    const v = value / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2]);
}

function contrast(a: Rgb, b: Rgb): number {
  const [x, y] = [relativeLuminance(a), relativeLuminance(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/** `ink`, moved toward white or black until it reads on every surface. */
function readable(ink: Rgb, surfaces: Rgb[], minimum: number): Rgb {
  const score = (color: Rgb) => Math.min(...surfaces.map((surface) => contrast(color, surface)));
  if (score(ink) >= minimum) return ink;
  const target = score(WHITE) > score(BLACK) ? WHITE : BLACK;
  for (let step = 1; step <= 20; step++) {
    const adjusted = mix(ink, target, step / 20);
    if (score(adjusted) >= minimum) return adjusted;
  }
  return target;
}

const same = (a: Rgb, b: Rgb) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2];

const DARK_ANSI = [
  0x2c2c31, 0xff6b63, 0x5fd68b, 0xffd166, 0x5aa9ff, 0xc792f6, 0x5fd4e8, 0xd6d6dd, 0x6d6d76,
  0xff8b84, 0x86e5a8, 0xffe08f, 0x86c1ff, 0xddb3ff, 0x8ce4f3, 0xf5f5f7,
];
const LIGHT_ANSI = [
  0x1d1d1f, 0xc9302a, 0x1f8a3d, 0x9a6700, 0x0a63d6, 0x9340c8, 0x0d7d92, 0x8e8e95, 0x6e6e75,
  0xe0453e, 0x2aa24b, 0xb97b00, 0x1e7bf0, 0xab55e4, 0x1994aa, 0x1d1d1f,
];

/** Graphite, Dusk or Light with an accent. */
function original(id: string, accent: Accent): Look {
  const dark = id !== "light";
  const pick = (name: Accent) => rgb(ACCENT_COLORS[name][dark ? 0 : 1]);
  const tone = pick(accent);
  const luminance = 0.299 * tone[0] + 0.587 * tone[1] + 0.114 * tone[2];
  const look: Look = {
    dark,
    bg: rgb(0x101012),
    chrome: rgb(0x1c1c1f),
    elevated: rgb(0x29292d),
    fg: rgb(0xececf1),
    secondary: rgb(0xa0a0a8),
    muted: rgb(0x6d6d76),
    accent: tone,
    onAccent: luminance > 170 ? INK : WHITE,
    green: rgb(0x30d158),
    yellow: rgb(0xffd60a),
    red: rgb(0xff5a52),
    // Attention is amber, and stays apart from an amber focus accent.
    attention: pick(accent === "orange" ? "yellow" : "orange"),
    terminalFg: rgb(0xececf1),
    cursor: tone,
    selection: BLACK,
    ansi: DARK_ANSI.map(rgb),
  };
  if (id === "dusk") {
    Object.assign(look, {
      bg: rgb(0x12111c),
      chrome: rgb(0x1e1c2b),
      elevated: rgb(0x2b2840),
      fg: rgb(0xe9e7f5),
      secondary: rgb(0xa29fb8),
      muted: rgb(0x6f6c87),
    });
    look.ansi[0] = rgb(0x2f2c42);
    look.ansi[8] = rgb(0x6f6c87);
  } else if (id === "light") {
    Object.assign(look, {
      bg: rgb(0xffffff),
      chrome: rgb(0xececef),
      elevated: rgb(0xffffff),
      fg: rgb(0x1d1d1f),
      secondary: rgb(0x5e5e66),
      muted: rgb(0x8e8e95),
      green: rgb(0x28a745),
      yellow: rgb(0xe0a800),
      red: rgb(0xe5372d),
      ansi: LIGHT_ANSI.map(rgb),
    });
  }
  look.terminalFg = look.fg;
  look.selection = mix(look.bg, look.accent, dark ? 0.34 : 0.24);
  return look;
}

/** A palette's 23 colours, in the order `ThemeColors::from_rgb` takes them. */
function parse(colors: string): Rgb[] {
  return Array.from({ length: 23 }, (_, index) =>
    rgb(Number.parseInt(colors.slice(index * 6, index * 6 + 6), 16)),
  );
}

/** A palette is dark when its background is. */
export function isDark(colors: string): boolean {
  const [r, g, b] = parse(colors)[0];
  return 299 * r + 587 * g + 114 * b < 128_000;
}

/**
 * Window materials derived from the palette the terminal uses. Terminal
 * colours stay exact; interface text and accents get a contrast floor.
 */
function imported(colors: string): Look {
  const [background, foreground, , cursor, cursorText, selection, , ...ansi] = parse(colors);
  const dark = isDark(colors);
  const look = original(dark ? "graphite" : "light", "blue");
  look.bg = background;
  look.terminalFg = foreground;
  look.cursor = cursor;
  look.cursorText = cursorText;
  look.selection = selection;
  look.ansi = ansi;
  const lift = dark ? WHITE : BLACK;
  const anchor = contrast(look.bg, WHITE) > contrast(look.bg, BLACK) ? WHITE : BLACK;
  const material = (amount: number) => {
    const surface = mix(look.bg, lift, amount);
    return contrast(anchor, surface) >= 4.5 ? surface : look.bg;
  };
  look.chrome = material(dark ? 0.055 : 0.035);
  look.elevated = material(dark ? 0.095 : 0.015);
  const surfaces = [look.bg, look.chrome, look.elevated];
  look.fg = readable(look.terminalFg, surfaces, 4.5);
  look.secondary = readable(mix(look.chrome, look.fg, 0.78), surfaces, 4.5);
  look.muted = readable(mix(look.chrome, look.fg, 0.6), surfaces, 4.5);
  look.accent = readable(ansi[4], surfaces, 3);
  look.onAccent = readable(look.bg, [look.accent], 4.5);
  look.green = readable(ansi[2], surfaces, 4.5);
  look.yellow = readable(ansi[3], surfaces, 4.5);
  look.red = readable(ansi[1], surfaces, 4.5);
  // Attention takes the palette's yellow, unless that is also its focus
  // colour; then it keeps the amber of the original themes.
  const amber = look.attention;
  look.attention = readable(ansi[3], surfaces, 3);
  if (same(look.attention, look.accent)) look.attention = readable(amber, surfaces, 3);
  return look;
}

export const lookOf = (theme: Theme, accent: Accent): Look =>
  theme.colors ? imported(theme.colors) : original(theme.id, accent);

/** Where a theme comes from, as Preferences names it. */
export const kindOf = (theme: Theme) => (theme.colors ? "iTerm2 collection" : "Neptune theme");

/**
 * The custom properties an imported palette sets over its base theme. The
 * original themes are declared in `globals.css` and need none.
 */
export function themeVariables(theme: Theme): Record<string, string> {
  if (!theme.colors) return {};
  const look = imported(theme.colors);
  const variables: Record<string, string> = {
    "--bg": hex(look.bg),
    "--chrome": hex(look.chrome),
    "--elevated": hex(look.elevated),
    "--fg": hex(look.fg),
    "--secondary": hex(look.secondary),
    "--muted": hex(look.muted),
    "--accent": hex(look.accent),
    "--on-accent": hex(look.onAccent),
    "--green": hex(look.green),
    "--yellow": hex(look.yellow),
    "--red": hex(look.red),
    // Label ink on a destructive control: white, unless the red is too light.
    "--on-red": contrast(WHITE, look.red) >= 3 ? "#ffffff" : hex(INK),
    "--attention": hex(look.attention),
    "--term-fg": hex(look.terminalFg),
    "--cursor": hex(look.cursor),
    "--selection": hex(look.selection),
    "--thumb": look.dark ? hex(mix(look.elevated, WHITE, 0.16)) : "#ffffff",
  };
  look.ansi.forEach((color, index) => {
    variables[`--ansi-${index}`] = hex(color);
  });
  return variables;
}
