"use client";
import { useEffect, useState } from "react";
import { PACKAGES, downloadPath, isMacDesktop, recommendPackage, type Channel, type PackageId, type Release } from "@/lib/releases";
import { REPO } from "@/lib/site";

export function Downloads({ channel, release, unavailable }: { channel: Channel; release: Release | null; unavailable: boolean }) {
  const [recommended, setRecommended] = useState<PackageId | null>(null);
  const [mac, setMac] = useState(false);
  useEffect(() => {
    let active = true;
    async function detect() {
      const ua = navigator.userAgent;
      const data = (navigator as Navigator & { userAgentData?: { getHighEntropyValues(hints: string[]): Promise<{ architecture?: string; bitness?: string }> } }).userAgentData;
      const hints = await data?.getHighEntropyValues(["architecture", "bitness"]).catch(() => undefined);
      if (active) {
        setMac(isMacDesktop(ua, navigator.maxTouchPoints));
        setRecommended(recommendPackage(ua, hints?.architecture, hints?.bitness, navigator.maxTouchPoints));
      }
    }
    void detect();
    return () => { active = false; };
  }, []);
  const button = "inline-flex min-h-11 items-center justify-center rounded-pane bg-accent px-5 py-3 font-medium text-on-accent hover:brightness-110";
  const selected = PACKAGES.find((pkg) => pkg.id === recommended);
  return (
    <div className="mx-auto w-full max-w-[940px] px-5 py-14 sm:px-8">
      <p className="text-sm font-medium text-secondary">{channel === "stable" ? "Stable releases" : "Beta releases"}</p>
      <h1 className="mt-3 text-[clamp(36px,6vw,60px)] leading-tight font-semibold tracking-[-0.04em]">Download Neptune{channel === "beta" ? " Beta" : ""}</h1>
      <p className="mt-4 max-w-xl text-secondary">A native terminal for macOS, Windows and Linux. Real shells, GPU rendering, and a quiet place to work.</p>
      <nav className="mt-6 flex gap-5 text-sm" aria-label="Release channel">
        <a href="/download" aria-current={channel === "stable" ? "page" : undefined} className="underline underline-offset-4">Stable</a>
        <a href="/download/beta" aria-current={channel === "beta" ? "page" : undefined} className="underline underline-offset-4">Beta</a>
      </nav>
      {release ? <>
        <div className="mt-10 rounded-window bg-chrome p-6 sm:p-8">
          <p className="text-sm text-secondary">Version {release.version} · {release.prerelease ? "Prerelease" : "Stable"}</p>
          {selected ? <div className="mt-5">
            <a className={button} href={downloadPath(channel, selected.id, release.tag)}>Download for {selected.id.startsWith("macos") ? "macOS" : selected.id.startsWith("windows") ? "Windows" : "Linux"}</a>
            <p className="mt-3 text-sm text-secondary">{selected.label} · {selected.detail}</p>
          </div> : <div className="mt-5">
            <p className="font-medium">{mac ? "Choose your Mac's chip" : "Choose your platform below"}</p>
            {mac && <p className="mt-2 text-sm text-secondary">Apple menu → About This Mac shows Chip (Apple Silicon) or Processor (Intel).</p>}
            {mac && <div className="mt-4 flex flex-wrap gap-3">
              <button className="min-h-11 rounded-control bg-control px-4 hover:bg-pressed" onClick={() => setRecommended("macos-arm64")}>Apple Silicon</button>
              <button className="min-h-11 rounded-control bg-control px-4 hover:bg-pressed" onClick={() => setRecommended("macos-x64")}>Intel</button>
            </div>}
          </div>}
          <p className="mt-5 text-sm text-secondary">{channel === "beta" ? "Beta builds may have rough edges. Stable is recommended for everyday work." : "Recommended for everyday work."}</p>
        </div>
        <h2 className="mt-10 text-xl font-semibold">All downloads</h2>
        <div className="mt-4 grid gap-3 sm:grid-cols-2">
          {PACKAGES.map((pkg) => {
            const asset = release.assets.find((a) => a.id === pkg.id)!;
            return <div key={pkg.id} className="min-w-0 rounded-pane bg-chrome p-5">
              <a className="inline-flex min-h-11 items-center gap-2 font-medium underline decoration-separator underline-offset-4 hover:text-accent" href={downloadPath(channel, pkg.id, release.tag)}>{pkg.label} <span aria-hidden>↓</span></a>
              <p className="text-sm text-secondary">{pkg.detail} · {(asset.size / 1024 ** 2).toFixed(1)} MB</p>
              <details className="mt-3 text-xs text-muted"><summary className="cursor-pointer">SHA-256</summary><code className="mt-2 block break-all select-text">{asset.sha256}</code></details>
            </div>;
          })}
        </div>
        <p className="mt-5 text-sm text-secondary">Requires a graphical desktop and graphics driver. Linux builds target glibc 2.35+ (Ubuntu 22.04+). AppImage includes window libraries; graphics drivers stay on your system. Windows installers are currently unsigned and may show a SmartScreen warning.</p>
        <section className="mt-10" aria-labelledby="release-notes">
          <h2 id="release-notes" className="text-xl font-semibold">What&apos;s New</h2>
          <div className="mt-4 space-y-3 text-secondary">
            {release.notes.split("\n").filter((line) => line.trim() && line.trim() !== "### What's New").map((line, index) => line.startsWith("### ")
              ? <h3 className="font-medium text-fg" key={index}>{line.slice(4)}</h3>
              : <p className="whitespace-pre-wrap" key={index}>{line.startsWith("- ") ? `• ${line.slice(2)}` : line}</p>)}
          </div>
        </section>
        <div className="mt-8 flex flex-wrap gap-x-6 gap-y-3 text-sm">
          <a className="underline underline-offset-4" href={release.url}>GitHub release</a>
          <a className="underline underline-offset-4" href={release.sourceUrl}>Source for this build</a>
          <a className="underline underline-offset-4" href={release.checksumsUrl}>SHA256SUMS</a>
          <a className="underline underline-offset-4" href={`${REPO}/blob/main/docs/releases.md#verify-a-download`}>Verify build provenance</a>
        </div>
      </> : <div role="status" className="mt-10 rounded-window bg-chrome p-7">
        <h2 className="text-xl font-semibold">{unavailable ? "Downloads are temporarily unavailable" : `No ${channel === "beta" ? "beta" : "stable"} release published yet`}</h2>
        <p className="mt-3 text-secondary">{unavailable ? "Please try again shortly or visit GitHub Releases." : "Completed releases will appear here after publication. You can build Neptune from source today."}</p>
        <a className="mt-5 inline-block underline underline-offset-4" href={`${REPO}/releases`}>GitHub Releases</a>
        <a className="ml-5 inline-block underline underline-offset-4" href={`${REPO}#build-and-run`}>Build from source</a>
      </div>}
    </div>
  );
}
