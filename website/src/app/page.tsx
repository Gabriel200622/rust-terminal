import { GitHubMark, Icon, NeptuneMark } from "@/components/icons";
import { Customize } from "@/components/site/customize";
import { HeroDemo } from "@/components/site/hero-demo";
import { Install } from "@/components/site/install";
import { Internals } from "@/components/site/internals";
import { REPO, doc } from "@/lib/site";

function Nav() {
  return (
    <header className="sticky top-0 z-50 border-b border-transparent bg-bg/75 backdrop-blur-xl">
      <nav className="mx-auto flex h-14 w-full max-w-[1180px] items-center justify-between px-5 sm:px-8">
        <a href="#top" className="flex items-center gap-2.5 rounded-control">
          <NeptuneMark size={24} />
          <span className="text-[15px] font-semibold tracking-[-0.01em]">Neptune</span>
        </a>
        <div className="flex items-center gap-1.5">
          <a
            href={REPO}
            className="flex h-[30px] items-center gap-2 rounded-control px-2.5 text-[13px] font-medium text-secondary transition-colors duration-100 hover:bg-hover hover:text-fg"
          >
            <GitHubMark size={15} />
            GitHub
          </a>
          <a
            href="#install"
            className="flex h-[30px] items-center rounded-control bg-control px-3 text-[13px] font-medium text-fg transition-colors duration-100 hover:bg-pressed"
          >
            Get Neptune
          </a>
        </div>
      </nav>
    </header>
  );
}

function Hero() {
  return (
    <section
      id="top"
      className="relative mx-auto flex w-full max-w-[1244px] flex-col items-center px-4 pt-12 sm:px-8 sm:pt-16"
    >
      {/* One quiet light behind the window, in the accent. */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute top-[38%] left-1/2 -z-10 h-[520px] w-[min(1100px,120%)] -translate-x-1/2 rounded-[50%] opacity-[0.16] blur-[110px]"
        style={{ background: "var(--accent)" }}
      />
      <h1 className="max-w-[15ch] animate-rise text-center text-[clamp(40px,6.6vw,72px)] leading-[0.98] font-semibold tracking-[-0.045em] text-balance">
        A native terminal for focused work.
      </h1>
      <p className="mt-6 max-w-[34rem] animate-rise text-center text-[clamp(16px,1.6vw,18px)] leading-[1.5] text-secondary [animation-delay:60ms]">
        GPU rendering, real shell sessions and a quiet workspace interface. Written in
        Rust, with no webview.
      </p>
      <div className="mt-8 flex animate-rise flex-wrap items-center justify-center gap-2.5 [animation-delay:120ms]">
        <a
          href="#install"
          className="flex h-10 items-center gap-2 rounded-pane bg-accent pr-4 pl-[18px] text-[14px] font-medium text-on-accent transition-[filter] duration-100 hover:brightness-110 active:brightness-90"
        >
          Build from source
          <Icon name="arrowDown" size={14} />
        </a>
        <a
          href={REPO}
          className="flex h-10 items-center gap-2 rounded-pane bg-control px-4 text-[14px] font-medium text-fg transition-colors duration-100 hover:bg-pressed"
        >
          <GitHubMark size={15} />
          View source
        </a>
      </div>
      <div className="mt-10 w-full animate-rise [animation-delay:180ms] sm:mt-12">
        <HeroDemo />
      </div>
    </section>
  );
}

function Footer() {
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

export default function Page() {
  return (
    <>
      <Nav />
      <main className="flex flex-col gap-32 overflow-x-clip sm:gap-40">
        <Hero />
        <Customize />
        <Internals />
        <Install />
      </main>
      <Footer />
    </>
  );
}
