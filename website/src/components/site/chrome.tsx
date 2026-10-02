import Link from "next/link";
import { GitHubMark, NeptuneMark } from "../icons";
import { REPO, doc } from "@/lib/site";

/** The navigation every page shares. */
export function Nav() {
  return (
    <header className="sticky top-0 z-50 border-b border-transparent bg-bg/75 backdrop-blur-xl">
      <nav className="mx-auto flex h-14 w-full max-w-[1180px] items-center justify-between px-5 sm:px-8">
        <Link href="/#top" className="flex items-center gap-2.5 rounded-control">
          <NeptuneMark size={24} />
          <span className="text-[15px] font-semibold tracking-[-0.01em]">Neptune</span>
        </Link>
        <div className="flex items-center gap-1.5">
          <a
            href={REPO}
            className="flex h-[30px] items-center gap-2 rounded-control px-2.5 text-[13px] font-medium text-secondary transition-colors duration-100 hover:bg-hover hover:text-fg"
          >
            <GitHubMark size={15} />
            GitHub
          </a>
          <a
            href="/download"
            className="flex h-[30px] items-center rounded-control bg-control px-3 text-[13px] font-medium text-fg transition-colors duration-100 hover:bg-pressed"
          >
            Get Neptune
          </a>
        </div>
      </nav>
    </header>
  );
}

export function Footer() {
  const links = [
    ["Source", REPO],
    ["Design", doc("docs/design.md")],
    ["Architecture", doc("docs/architecture.md")],
    ["Performance", doc("docs/performance.md")],
  ] as const;
  return (
    <footer className="mx-auto mt-32 w-full max-w-[1180px] px-5 pb-10 sm:px-8">
      <div className="flex flex-col gap-5 border-t border-separator pt-7 text-[12.5px] text-muted sm:flex-row sm:items-center sm:justify-between">
        <p className="flex items-center gap-2.5">
          <NeptuneMark size={18} />
          <span>
            <span className="font-medium text-secondary">Neptune</span> · MIT licensed
          </span>
        </p>
        <ul className="flex flex-wrap gap-x-5 gap-y-2">
          {links.map(([label, href]) => (
            <li key={label}>
              <a href={href} className="transition-colors duration-100 hover:text-fg">
                {label}
              </a>
            </li>
          ))}
        </ul>
      </div>
    </footer>
  );
}

/** The quiet accent light behind a page's opening. */
export function Glow({ className = "top-[38%] h-[520px]" }: { className?: string }) {
  return (
    <div
      aria-hidden="true"
      className={`pointer-events-none absolute left-1/2 -z-10 w-[min(1100px,120%)] -translate-x-1/2 rounded-[50%] opacity-[0.16] blur-[110px] ${className}`}
      style={{ background: "var(--accent)" }}
    />
  );
}
