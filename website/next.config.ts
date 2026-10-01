import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // The landing page is static: `bun run build` writes plain files to `out/`.
  output: "export",
};

export default nextConfig;
