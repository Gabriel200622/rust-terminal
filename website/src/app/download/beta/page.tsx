import { Suspense } from "react";
import { Downloads } from "@/components/site/download";
import { getRelease } from "@/lib/github-releases";

export const metadata = { title: "Download Neptune Beta" };
async function Beta() { return <Downloads channel="beta" {...await getRelease("beta")} />; }
export default function Page() { return <Suspense fallback={<p className="p-8">Loading beta downloads…</p>}><Beta /></Suspense>; }
