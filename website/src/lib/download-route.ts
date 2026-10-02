import { getRelease } from "./github-releases";
import { PACKAGES, assetUrl, type Channel } from "./releases";

export async function downloadResponse(request: Request, platform: string, channel: Channel, resolveRelease: typeof getRelease = getRelease) {
  if (!PACKAGES.some((pkg) => pkg.id === platform)) return new Response("Unknown package", { status: 404 });
  const tag = new URL(request.url).searchParams.get("tag") ?? undefined;
  const { release, unavailable } = await resolveRelease(channel, tag);
  const asset = release?.assets.find((asset) => asset.id === platform);
  if (!release || !asset) return new Response(unavailable ? "Release service temporarily unavailable" : "No published release available", { status: unavailable ? 503 : 404, headers: { "Retry-After": "60", "Cache-Control": "no-store" } });
  // 307 is deliberately temporary: a stable Neptune-owned route can advance.
  return new Response(null, { status: 307, headers: { Location: assetUrl(release.tag, asset.name), "Cache-Control": "public, max-age=60" } });
}
