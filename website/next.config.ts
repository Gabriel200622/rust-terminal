import type { NextConfig } from "next";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const nextConfig: NextConfig = {
  // Vercel hosts Next.js server routes; binaries remain on GitHub Releases.
  cacheComponents: true,
  env: {
    // Public trust anchor, shared with the Rust desktop; never a secret.
    NEPTUNE_UPDATE_PUBLIC_KEY: readFileSync(resolve(__dirname, "../packaging/update-public-key.hex"), "utf8").trim(),
  },
};

export default nextConfig;
