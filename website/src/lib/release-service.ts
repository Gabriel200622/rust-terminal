import { createPublicKey, verify } from "node:crypto";
import { assetUrl, parseVersion, releaseFromManifest, selectRelease, type Channel, type Release } from "./releases";

const API = "https://api.github.com/repos/zevem/neptune/releases";
export type ReleaseResult = { release: Release | null; unavailable: boolean };
type ReadRelease = (url: string, api?: boolean) => Promise<Response>;

async function boundedText(response: Response, limit: number) {
  if (!response.ok || Number(response.headers.get("content-length")) > limit || !response.body) throw new Error("Release service unavailable");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      size += value.length;
      if (size > limit) throw new Error("Release metadata exceeds budget");
      chunks.push(value);
    }
  } finally { await reader.cancel(); }
  return Buffer.concat(chunks).toString("utf8");
}

export function verifyMetadata(bytes: Buffer, signature: string, publicKey: string) {
  if (!/^[0-9a-f]{64}$/.test(publicKey) || !/^[0-9a-f]{128}$/.test(signature.trim())) throw new Error("Invalid update signature format");
  const key = createPublicKey({ key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), Buffer.from(publicKey, "hex")]), format: "der", type: "spki" });
  if (!verify(null, bytes, key, Buffer.from(signature.trim(), "hex"))) throw new Error("Invalid update signature");
}

// This seam lets tests exercise actual API resolution/signature checks offline.
export async function loadRelease(channel: Channel, tag: string | undefined, read: ReadRelease, publicKey: string): Promise<ReleaseResult> {
  try {
    if (tag && (tag.length > 120 || !tag.startsWith("v") || !parseVersion(tag.slice(1)))) throw new Error("Invalid release tag");
    let selected;
    if (tag || channel === "stable") {
      const response = await read(tag ? `${API}/tags/${encodeURIComponent(tag)}` : `${API}/latest`, true);
      if (response.status === 404) return { release: null, unavailable: false };
      const data = JSON.parse(await boundedText(response, 1024 * 1024));
      selected = channel === "stable" ? selectRelease(data, channel) : selectRelease([data], channel);
    } else {
      const records: unknown[] = [];
      for (let page = 1; page <= 10; page++) {
        const batch = JSON.parse(await boundedText(await read(`${API}?per_page=100&page=${page}`, true), 4 * 1024 * 1024));
        if (!Array.isArray(batch)) throw new Error("Invalid releases response");
        records.push(...batch);
        if (batch.length < 100) break;
        if (page === 10) throw new Error("Release history exceeds discovery budget");
      }
      selected = selectRelease(records, channel);
    }
    if (!selected || tag && selected.tag_name !== tag) return { release: null, unavailable: false };
    const [manifest, signature] = await Promise.all([
      read(assetUrl(selected.tag_name, "update-manifest.json")).then((response) => boundedText(response, 128 * 1024)),
      read(assetUrl(selected.tag_name, "update-manifest.sig")).then((response) => boundedText(response, 256)),
    ]);
    verifyMetadata(Buffer.from(manifest), signature, publicKey);
    return { release: releaseFromManifest(selected, JSON.parse(manifest)), unavailable: false };
  } catch {
    return { release: null, unavailable: true };
  }
}
