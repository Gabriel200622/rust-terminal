import type { Metadata } from "next";
import { Suspense } from "react";
import { Footer, Nav } from "@/components/site/chrome";
import { Downloads, DownloadsLoading } from "@/components/site/download";
import { getRelease } from "@/lib/github-releases";

export const metadata: Metadata = {
  title: "Download Neptune",
  description:
    "Download Neptune, a native Rust terminal, for macOS, Windows and Linux. Every package with its checksum and build provenance.",
  alternates: { canonical: "/download" },
};

async function Stable() {
  return <Downloads channel="stable" {...await getRelease("stable")} />;
}

export default function Page() {
  return (
    <>
      <Nav />
      <main className="overflow-x-clip">
        <Suspense fallback={<DownloadsLoading channel="stable" />}>
          <Stable />
        </Suspense>
      </main>
      <Footer />
    </>
  );
}
