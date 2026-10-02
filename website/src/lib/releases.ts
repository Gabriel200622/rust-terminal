import { REPO } from "./site";

export type Channel = "stable" | "beta";
export const PACKAGES = [
  { id: "macos-arm64", label: "macOS Apple Silicon", detail: "ARM64 · DMG", suffix: "macos-arm64.dmg" },
  { id: "macos-x64", label: "macOS Intel", detail: "x64 · DMG", suffix: "macos-x64.dmg" },
  { id: "windows-x64", label: "Windows x64", detail: "x64 · EXE installer · unsigned", suffix: "windows-x64.exe" },
  { id: "linux-x64-appimage", label: "Linux AppImage", detail: "x64 · portable AppImage", suffix: "linux-x64.AppImage" },
  { id: "linux-x64-deb", label: "Linux DEB", detail: "x64 · Debian / Ubuntu", suffix: "linux-x64.deb" },
] as const;
export type PackageId = typeof PACKAGES[number]["id"];
export type Release = {
  tag: string; version: string; prerelease: boolean; notes: string;
  url: string; sourceUrl: string; checksumsUrl: string;
  assets: { id: PackageId; name: string; size: number; sha256: string }[];
};

const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;
export function parseVersion(version: string) {
  const match = SEMVER.exec(version);
  if (!match || match[0] !== version) return null;
  const pre = match[4]?.split(".") ?? [];
  if (pre.some((part) => /^\d+$/.test(part) && part.length > 1 && part[0] === "0")) return null;
  return { core: match.slice(1, 4), pre };
}

type ReleaseRecord = { tag_name: string; prerelease: boolean; published_at: string; assets: { name: string }[] };
function record(value: unknown): ReleaseRecord | null {
  if (!value || typeof value !== "object") return null;
  const r = value as Record<string, unknown>;
  if (r.draft !== false || typeof r.tag_name !== "string" || !r.tag_name.startsWith("v") || typeof r.prerelease !== "boolean" || typeof r.published_at !== "string" || !Number.isFinite(Date.parse(r.published_at)) || !Array.isArray(r.assets)) return null;
  const version = parseVersion(r.tag_name.slice(1));
  if (!version || r.prerelease !== (version.pre.length > 0)) return null;
  if (!r.assets.every((asset) => asset && typeof asset === "object" && typeof asset.name === "string")) return null;
  return r as ReleaseRecord;
}

export function selectRelease(data: unknown, channel: Channel): ReleaseRecord | null {
  if (channel === "stable") {
    const release = record(data);
    return release && !release.prerelease ? release : null;
  }
  if (!Array.isArray(data)) throw new Error("Invalid release list");
  return data.map(record).filter((r): r is ReleaseRecord => !!r && r.prerelease).sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at))[0] ?? null;
}

export function assetUrl(tag: string, name: string) {
  return `${REPO}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;
}

export function releaseFromManifest(selected: ReleaseRecord, value: unknown): Release {
  if (!value || typeof value !== "object") throw new Error("Invalid manifest");
  const m = value as Record<string, unknown>;
  if (m.schema !== 1 || m.repository !== "zevem/neptune" || m.tag !== selected.tag_name || m.version !== selected.tag_name.slice(1) || typeof m.notes !== "string" || !m.notes.trim() || m.notes.length > 32000 || typeof m.commit !== "string" || !/^[0-9a-f]{40}$/.test(m.commit) || !Array.isArray(m.assets) || m.assets.length !== PACKAGES.length) throw new Error("Manifest identity mismatch");
  const assets = PACKAGES.map((pkg) => {
    const name = `Neptune-${m.version}-${pkg.suffix}`;
    const matches = (m.assets as Record<string, unknown>[]).filter((a) => a?.platform === pkg.id);
    const asset = matches[0];
    if (matches.length !== 1 || asset.name !== name || typeof asset.size !== "number" || !Number.isSafeInteger(asset.size) || asset.size <= 0 || asset.size > 1024 ** 3 || typeof asset.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(asset.sha256) || selected.assets.filter((a) => a.name === name).length !== 1) throw new Error("Incomplete or invalid release artifacts");
    return { id: pkg.id, name, size: asset.size, sha256: asset.sha256 };
  });
  for (const name of ["SHA256SUMS", "update-manifest.json", "update-manifest.sig"]) {
    if (selected.assets.filter((a) => a.name === name).length !== 1) throw new Error("Missing release integrity metadata");
  }
  return { tag: selected.tag_name, version: m.version as string, prerelease: selected.prerelease, notes: m.notes, url: `${REPO}/releases/tag/${encodeURIComponent(selected.tag_name)}`, sourceUrl: `${REPO}/tree/${m.commit}`, checksumsUrl: assetUrl(selected.tag_name, "SHA256SUMS"), assets };
}

function mobileBrowser(userAgent: string, touchPoints: number) {
  return /Android|iPhone|iPad|Mobile/i.test(userAgent) || /Macintosh/i.test(userAgent) && touchPoints > 1;
}

export function isMacDesktop(userAgent: string, touchPoints = 0) {
  return !mobileBrowser(userAgent, touchPoints) && /Macintosh|Mac OS X/i.test(userAgent);
}

export function recommendPackage(userAgent: string, architecture?: string, bitness?: string, touchPoints = 0): PackageId | null {
  // UA strings usually hide Apple Silicon. Recommend ARM only when known;
  // ambiguous Macs get an explicit chip selector, never an architecture guess.
  if (mobileBrowser(userAgent, touchPoints) || bitness === "32") return null;
  if (isMacDesktop(userAgent, touchPoints)) {
    if (/arm|aarch64/i.test(architecture ?? "")) return "macos-arm64";
    if (/x86|x64|amd64/i.test(architecture ?? "")) return "macos-x64";
    return null;
  }
  if (/arm|aarch64/i.test(`${architecture ?? ""} ${userAgent}`) || bitness === "32") return null;
  if (/Windows/i.test(userAgent) && (/Win64|x64|WOW64/i.test(userAgent) || bitness === "64")) return "windows-x64";
  if (/Linux/i.test(userAgent) && (/x86_64|amd64/i.test(userAgent) || bitness === "64")) return "linux-x64-appimage";
  return null;
}

export function downloadPath(channel: Channel, pkg: PackageId, tag?: string) {
  return `/download/${channel === "beta" ? "beta/" : ""}file/${pkg}${tag ? `?tag=${encodeURIComponent(tag)}` : ""}`;
}
