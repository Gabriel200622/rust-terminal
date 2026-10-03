import type { Metadata } from "next";
import { Suspense } from "react";
import { Footer, Nav } from "@/components/site/chrome";
import { Downloads, DownloadsLoading } from "@/components/site/download";
import { getDownloadPage } from "@/lib/download-page";

export const metadata: Metadata = {
  title: "Download Neptune Beta",
  description:
    "Early builds of Neptune, a native Rust terminal, for macOS, Windows and Linux. Every package with its checksum and build provenance.",
  alternates: { canonical: "/download/beta" },
};

async function Beta() {
  return <Downloads {...await getDownloadPage("beta")} />;
}

export default function Page() {
  return (
    <>
      <Nav />
      <main className="overflow-x-clip">
        <Suspense fallback={<DownloadsLoading channel="beta" />}>
          <Beta />
        </Suspense>
      </main>
      <Footer />
    </>
  );
}
