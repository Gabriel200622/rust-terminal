import { getRelease } from "./github-releases";
import type { ReleaseResult } from "./release-service";
import type { Channel } from "./releases";

/** Prefer Stable once published, keeping a confirmed empty channel out of the UI. */
export async function getDownloadPage(
  requested: Channel,
  resolveRelease: (channel: Channel) => Promise<ReleaseResult> = getRelease,
) {
  const stablePromise = resolveRelease("stable");
  if (requested === "beta") {
    const [stable, beta] = await Promise.all([stablePromise, resolveRelease("beta")]);
    return { channel: "beta" as const, stableAvailable: Boolean(stable.release), ...beta };
  }

  const stable = await stablePromise;
  // A failed lookup does not establish that no Stable release has been published.
  if (stable.release || stable.unavailable) {
    return { channel: "stable" as const, stableAvailable: Boolean(stable.release), ...stable };
  }
  return { channel: "beta" as const, stableAvailable: false, ...await resolveRelease("beta") };
}
