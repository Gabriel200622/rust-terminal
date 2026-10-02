import { test } from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync, sign } from "node:crypto";
import { PACKAGES, assetUrl, downloadPath, isMacDesktop, parseVersion, recommendPackage, selectRelease } from "../src/lib/releases";
import { downloadResponse } from "../src/lib/download-route";
import { loadRelease } from "../src/lib/release-service";

const { publicKey, privateKey } = generateKeyPairSync("ed25519");
const trust = publicKey.export({ type: "spki", format: "der" }).subarray(-32).toString("hex");
function fixture(version: string, published = "2026-10-01T00:00:00Z") {
  const tag = `v${version}`;
  const assets = PACKAGES.map((pkg) => ({ name: `Neptune-${version}-${pkg.suffix}`, platform: pkg.id, size: 1234, sha256: "ab".repeat(32) }));
  const manifest = JSON.stringify({ schema: 1, repository: "zevem/neptune", tag, version, commit: "a".repeat(40), notes: "### What's New\n- Faster startup.", assets });
  const signature = sign(null, Buffer.from(manifest), privateKey).toString("hex");
  const record = { tag_name: tag, prerelease: version.split("+")[0].includes("-"), draft: false, published_at: published, assets: [...assets, ...["SHA256SUMS", "update-manifest.json", "update-manifest.sig"].map((name) => ({ name }))] };
  return { tag, record, manifest, signature };
}
function reader(records: unknown, f: ReturnType<typeof fixture>, urls: string[] = []) {
  return async (url: string) => {
    urls.push(url);
    if (url.endsWith("update-manifest.json")) return new Response(f.manifest);
    if (url.endsWith("update-manifest.sig")) return new Response(f.signature);
    return Response.json(records);
  };
}

test("strict SemVer supports prereleases and build metadata", () => {
  for (const version of ["0.1.0", "0.2.0-beta.1", "0.2.0-rc.1+build.5"]) assert.ok(parseVersion(version));
  for (const version of ["v0.1.0", "01.0.0", "0.1.0-beta.01", "0.1.0-", "0.1.0+", "0.1.0-a..b", "0.1.0\n"]) assert.equal(parseVersion(version), null);
});

test("stable uses latest published stable endpoint and verifies all signed assets", async () => {
  const f = fixture("0.2.0");
  const urls: string[] = [];
  const result = await loadRelease("stable", undefined, reader(f.record, f, urls), trust);
  assert.equal(result.release?.version, "0.2.0");
  assert.equal(result.release?.assets.length, 5);
  assert.equal(urls[0], "https://api.github.com/repos/zevem/neptune/releases/latest");
  assert.equal(selectRelease(fixture("0.3.0-beta.1").record, "stable"), null);
  assert.equal(selectRelease({ ...f.record, draft: true }, "stable"), null);
});

test("beta selects newest published prerelease, excluding stable and draft records", async () => {
  const newest = fixture("0.3.0-beta.2", "2026-10-02T00:00:00Z");
  const records = [fixture("0.4.0", "2026-10-03T00:00:00Z").record, fixture("0.3.0-beta.1").record, newest.record, { ...fixture("0.4.0-beta.1").record, draft: true }];
  const urls: string[] = [];
  const result = await loadRelease("beta", undefined, reader(records, newest, urls), trust);
  assert.equal(result.release?.version, "0.3.0-beta.2");
  assert.ok(urls[0].includes("per_page=100&page=1"));
});

test("beta searches later pages rather than treating a page of stables as empty", async () => {
  const beta = fixture("0.3.0-beta.1");
  let page = 0;
  const fallback = reader([beta.record], beta);
  const result = await loadRelease("beta", undefined, async (url) => {
    if (url.includes("?per_page")) {
      page++;
      return Response.json(page === 1 ? Array(100).fill(fixture("0.2.0").record) : [beta.record]);
    }
    return fallback(url);
  }, trust);
  assert.equal(page, 2);
  assert.equal(result.release?.version, "0.3.0-beta.1");
});

test("no releases, API failures, forged metadata and incomplete releases fail safely", async () => {
  assert.deepEqual(await loadRelease("stable", undefined, async () => new Response(null, { status: 404 }), trust), { release: null, unavailable: false });
  assert.deepEqual(await loadRelease("beta", undefined, async () => Response.json([]), trust), { release: null, unavailable: false });
  assert.equal((await loadRelease("stable", undefined, async () => new Response(null, { status: 403 }), trust)).unavailable, true);
  const f = fixture("0.2.0");
  assert.equal((await loadRelease("stable", undefined, reader(f.record, { ...f, manifest: f.manifest.replace("Faster", "Forged") }), trust)).unavailable, true);
  assert.equal((await loadRelease("stable", undefined, reader({ ...f.record, assets: f.record.assets.slice(1) }, f), trust)).unavailable, true);
  assert.equal((await loadRelease("stable", "v0.3.0-beta.1", reader(fixture("0.3.0-beta.1").record, fixture("0.3.0-beta.1")), trust)).release, null);
});

test("OS recommendations use architecture hints and preserve manual choices for ambiguity", () => {
  assert.equal(recommendPackage("Macintosh Mac OS X", "arm", "64"), "macos-arm64");
  assert.equal(recommendPackage("Macintosh Mac OS X", "x86", "64"), "macos-x64");
  assert.equal(recommendPackage("Macintosh Mac OS X"), null);
  assert.equal(recommendPackage("Windows NT 10.0; Win64; x64"), "windows-x64");
  assert.equal(recommendPackage("Windows NT 10.0", "arm", "64"), null);
  assert.equal(recommendPackage("Linux x86_64"), "linux-x64-appimage");
  assert.equal(recommendPackage("Linux aarch64"), null);
  assert.equal(recommendPackage("iPhone Mac OS X Mobile"), null);
  assert.equal(isMacDesktop("iPhone Mac OS X Mobile"), false);
  assert.equal(isMacDesktop("Macintosh Mac OS X", 5), false);
  assert.equal(recommendPackage("Macintosh Mac OS X", "arm", "64", 5), null);
  assert.equal(recommendPackage("Linux Android"), null);
  assert.equal(PACKAGES.length, 5);
});

test("Neptune-owned routes bind visible version and GitHub remains binary host", () => {
  assert.equal(downloadPath("stable", "macos-arm64", "v0.2.0"), "/download/file/macos-arm64?tag=v0.2.0");
  assert.equal(downloadPath("beta", "linux-x64-deb"), "/download/beta/file/linux-x64-deb");
  assert.equal(assetUrl("v0.2.0-beta.1", "Neptune-0.2.0-beta.1-linux-x64.deb"), "https://github.com/zevem/neptune/releases/download/v0.2.0-beta.1/Neptune-0.2.0-beta.1-linux-x64.deb");
});

test("download handlers redirect verified packages and fail closed for unavailable or unknown assets", async () => {
  for (const channel of ["stable", "beta"] as const) {
    const f = fixture(channel === "stable" ? "0.2.0" : "0.3.0-beta.1");
    const request = new Request(`https://neptune.rs/download/file/windows-x64?tag=${f.tag}`);
    const response = await downloadResponse(request, "windows-x64", channel, (ch, tag) => loadRelease(ch, tag, reader(f.record, f), trust));
    assert.equal(response.status, 307);
    assert.equal(response.headers.get("Location"), assetUrl(f.tag, `Neptune-${f.tag.slice(1)}-windows-x64.exe`));
  }
  const request = new Request("https://neptune.rs/download/file/linux-x64-deb");
  const unavailable = await downloadResponse(request, "linux-x64-deb", "stable", async () => ({ release: null, unavailable: true }));
  assert.equal(unavailable.status, 503);
  assert.equal(unavailable.headers.get("Cache-Control"), "no-store");
  const unknown = await downloadResponse(request, "unknown", "stable", async () => { throw new Error("Unknown package must not request metadata"); });
  assert.equal(unknown.status, 404);
});
