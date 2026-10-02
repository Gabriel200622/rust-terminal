import { Suspense } from "react";
import { Downloads } from "@/components/site/download";
import { getRelease } from "@/lib/github-releases";

export const metadata = { title: "Download Neptune" };
async function Stable() { return <Downloads channel="stable" {...await getRelease("stable")} />; }
export default function Page() { return <Suspense fallback={<p className="p-8">Loading downloads…</p>}><Stable /></Suspense>; }
