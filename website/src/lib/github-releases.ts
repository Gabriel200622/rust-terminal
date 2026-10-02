import { cacheLife } from "next/cache";
import { loadRelease, type ReleaseResult } from "./release-service";
import { type Channel } from "./releases";

async function read(url: string, api = false): Promise<Response> {
  // Credentials go only to the fixed API origin, never release asset redirects.
  return fetch(url, {
    headers: { Accept: api ? "application/vnd.github+json" : "application/octet-stream", "User-Agent": "Neptune-downloads", ...(api && process.env.GITHUB_RELEASES_TOKEN ? { Authorization: `Bearer ${process.env.GITHUB_RELEASES_TOKEN}` } : {}) },
    signal: AbortSignal.timeout(10000),
    next: { revalidate: 300 },
  });
}

export async function getRelease(channel: Channel, tag?: string): Promise<ReleaseResult> {
  "use cache";
  const result = await loadRelease(channel, tag, read, process.env.NEPTUNE_UPDATE_PUBLIC_KEY ?? "");
  // Cache failures and empty history as well as successful metadata. No client
  // API requests, no GitHub traffic on every render, and no cached draft assets.
  cacheLife(result.release ? { stale: 300, revalidate: 300, expire: 3600 } : { stale: 60, revalidate: 60, expire: 300 });
  return result;
}
