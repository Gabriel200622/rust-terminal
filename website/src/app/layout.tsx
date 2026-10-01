import type { Metadata, Viewport } from "next";
import { Geist, JetBrains_Mono } from "next/font/google";
import { PrefsProvider } from "@/components/prefs";
import "./globals.css";

// The same faces the app bundles: Geist for the interface, JetBrains Mono for
// terminal cells.
const geist = Geist({ variable: "--font-geist", subsets: ["latin"] });
const jetbrains = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
});

const description =
  "Pace is a native Rust terminal for focused work: GPU rendering, real shell sessions, and a quiet workspace interface. No webview.";

export const metadata: Metadata = {
  title: "Pace — a native terminal for focused work",
  description,
  openGraph: {
    title: "Pace — a native terminal for focused work",
    description,
    type: "website",
  },
};

export const viewport: Viewport = {
  themeColor: "#101012",
  colorScheme: "dark light",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="en"
      data-theme="graphite"
      data-accent="blue"
      data-cursor="block"
      data-blink="false"
      className={`${geist.variable} ${jetbrains.variable}`}
    >
      <body className="min-h-dvh">
        <PrefsProvider>{children}</PrefsProvider>
      </body>
    </html>
  );
}
