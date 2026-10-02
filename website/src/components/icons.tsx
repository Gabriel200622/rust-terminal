import Image from "next/image";

// Neptune's own icon set, ported from `src/icons.rs`: a 24-point grid, a
// 1.5-point stroke that does not scale with the glyph, and rounded ends.

const TAU = Math.PI * 2;
const fixed = (value: number) => Number(value.toFixed(2));
const path = (points: number[][], close = false) =>
  points
    .map(([x, y], index) => `${index ? "L" : "M"}${fixed(x)} ${fixed(y)}`)
    .join("") + (close ? "Z" : "");

const gear = (() => {
  const outline: number[][] = [];
  for (let tooth = 0; tooth < 8; tooth++) {
    for (const [offset, radius] of [
      [0, 7.5],
      [0.2, 9.5],
      [0.55, 9.5],
      [0.75, 7.5],
    ]) {
      const angle = ((tooth + offset) * TAU) / 8;
      outline.push([
        12 + radius * Math.cos(angle),
        12 + radius * Math.sin(angle),
      ]);
    }
  }
  return path(outline, true);
})();

const moon = (() => {
  const crescent: number[][] = [];
  for (let step = 0; step <= 20; step++) {
    const angle = ((-93 - step * 13.2) * Math.PI) / 180;
    crescent.push([12 + 9 * Math.cos(angle), 12 + 9 * Math.sin(angle)]);
  }
  for (let step = 0; step <= 12; step++) {
    const angle = ((65.2 + step * (139.6 / 12)) * Math.PI) / 180;
    crescent.push([18 + 7.125 * Math.cos(angle), 6 + 7.125 * Math.sin(angle)]);
  }
  return path(crescent, true);
})();

const refresh = (() => {
  const ring: number[][] = [];
  for (let step = 0; step <= 24; step++) {
    const angle = ((-60 + step * 12.5) * Math.PI) / 180;
    ring.push([12 + 8 * Math.cos(angle), 12 + 8 * Math.sin(angle)]);
  }
  return path(ring);
})();

const sun = Array.from({ length: 8 }, (_, index) => {
  const angle = (index * TAU) / 8;
  const [x, y] = [Math.cos(angle), Math.sin(angle)];
  return path([
    [12 + x * 7, 12 + y * 7],
    [12 + x * 9.5, 12 + y * 9.5],
  ]);
}).join("");

const frame = <rect x="3" y="4" width="18" height="16" rx="3" />;
const dot = (x: number, y: number, r: number) => (
  <circle key={`${x}-${y}`} cx={x} cy={y} r={r} fill="currentColor" stroke="none" />
);

const GLYPHS = {
  terminal: (
    <>
      <rect x="3" y="5" width="18" height="14" rx="3" />
      <path d="M7 9l3 3-3 3M13 15h4" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  close: <path d="M6 6l12 12M18 6L6 18" />,
  chevronRight: <path d="M9 5l7 7-7 7" />,
  chevronDown: <path d="M5 9l7 7 7-7" />,
  settings: (
    <>
      <path d={gear} />
      <circle cx="12" cy="12" r="3" />
    </>
  ),
  search: (
    <>
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="M15.3 15.3L20 20" />
    </>
  ),
  sidebar: (
    <>
      {frame}
      <path d="M8.5 4.5v15" />
    </>
  ),
  splitVertical: (
    <>
      {frame}
      <path d="M12 4.5v15" />
    </>
  ),
  splitHorizontal: (
    <>
      {frame}
      <path d="M3.5 12h17" />
    </>
  ),
  grid: (
    <>
      {[4, 14].flatMap((y) =>
        [4, 14].map((x) => (
          <rect key={`${x}-${y}`} x={x} y={y} width="6" height="6" rx="1.5" />
        )),
      )}
    </>
  ),
  check: <path d="M5 12l5 5L20 7" />,
  arrowUpRight: <path d="M6 18L18 6M7 6h11v11" />,
  arrowUp: <path d="M12 19V5M6 11l6-6 6 6" />,
  arrowDown: <path d="M12 5v14M6 13l6 6 6-6" />,
  arrowRight: <path d="M5 12h14M13 6l6 6-6 6" />,
  minus: <path d="M5 12h14" />,
  maximize: <path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5" />,
  minimize: <path d="M4 9h5V4M15 4v5h5M20 15h-5v5M9 20v-5H4" />,
  command: (
    <>
      <rect x="8" y="8" width="8" height="8" />
      {[
        [3, 3],
        [16, 3],
        [3, 16],
        [16, 16],
      ].map(([x, y]) => (
        <rect key={`${x}-${y}`} x={x} y={y} width="5" height="5" rx="2.5" />
      ))}
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d={sun} />
    </>
  ),
  moon: <path d={moon} />,
  copy: (
    <>
      <rect x="8" y="8" width="12" height="13" rx="2.5" />
      <path d="M15 5V3H4L3 4v11h2" />
    </>
  ),
  ellipsis: <>{[5.5, 12, 18.5].map((x) => dot(x, 12, 1.6))}</>,
  refresh: (
    <>
      <path d={refresh} />
      <path d="M16 2.5l.2 3.1 3.2-.4" />
    </>
  ),
  pencil: <path d="M4 20l1-4.5L16 4.5 19.5 8 8.5 19 4 20zM13.5 7l3.5 3.5" />,
  clipboard: (
    <>
      <rect x="5" y="5" width="14" height="16" rx="2.5" />
      <rect x="9" y="3" width="6" height="4" rx="1.5" />
      <path d="M9 12h6M9 16h4" />
    </>
  ),
  eraser: <path d="M8 19l-4.5-4.5 10-10L20 11l-8 8H8zM8.5 9.5L15 16M12 19h8" />,
  textSize: <path d="M3 19L8 7l5 12M4.8 15h6.4M15 19l3-7 3 7M16.2 16.5h3.6" />,
  swap: <path d="M4 8h16M16 4l4 4-4 4M20 16H4M8 12l-4 4 4 4" />,
  globe: (
    <>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18" />
      <ellipse cx="12" cy="12" rx="4" ry="9" />
    </>
  ),
  // Site-only glyphs, drawn to the same grid and stroke.
  play: <path d="M8 5.5v13l10.5-6.5L8 5.5z" />,
  pause: <path d="M8.5 5.5v13M15.5 5.5v13" />,
  lock: (
    <>
      <rect x="5" y="10.5" width="14" height="9.5" rx="2.5" />
      <path d="M8.5 10.5V8a3.5 3.5 0 017 0v2.5" />
    </>
  ),
} as const;

export type IconName = keyof typeof GLYPHS;

export function Icon({
  name,
  size = 16,
  className,
}: {
  name: IconName;
  size?: number;
  className?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      vectorEffect="non-scaling-stroke"
      aria-hidden="true"
      className={`shrink-0 [&_*]:[vector-effect:non-scaling-stroke] ${className ?? ""}`}
    >
      {GLYPHS[name]}
    </svg>
  );
}

/** The supplied logo, exported from `assets/branding/neptune-logo.png`. */
export function NeptuneMark({ size = 28 }: { size?: number }) {
  return (
    <Image
      src="/neptune-logo.png"
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      className="shrink-0"
      unoptimized
    />
  );
}

export function GitHubMark({ size = 16 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="currentColor"
      aria-hidden="true"
      className="shrink-0"
    >
      <path d="M8 0a8 8 0 00-2.53 15.59c.4.07.55-.17.55-.38v-1.33c-2.23.48-2.7-1.07-2.7-1.07-.36-.93-.89-1.17-.89-1.17-.73-.5.05-.49.05-.49.8.06 1.23.83 1.23.83.72 1.22 1.87.87 2.33.66.07-.52.28-.87.5-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82a7.6 7.6 0 014 0c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.28.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48v2.19c0 .21.15.46.55.38A8 8 0 008 0z" />
    </svg>
  );
}
