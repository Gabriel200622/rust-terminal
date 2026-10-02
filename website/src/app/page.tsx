import { GitHubMark, Icon } from "@/components/icons";
import { Footer, Glow, Nav } from "@/components/site/chrome";
import { Customize } from "@/components/site/customize";
import { HeroDemo } from "@/components/site/hero-demo";
import { Install } from "@/components/site/install";
import { Internals } from "@/components/site/internals";
import { REPO } from "@/lib/site";

function Hero() {
  return (
    <section
      id="top"
      className="relative mx-auto flex w-full max-w-[1244px] flex-col items-center px-4 pt-12 sm:px-8 sm:pt-16"
    >
      {/* One quiet light behind the window, in the accent. */}
      <Glow />
      <h1 className="max-w-[15ch] animate-rise text-center text-[clamp(40px,6.6vw,72px)] leading-[0.98] font-semibold tracking-[-0.045em] text-balance">
        A native terminal for focused work.
      </h1>
      <p className="mt-6 max-w-[34rem] animate-rise text-center text-[clamp(16px,1.6vw,18px)] leading-[1.5] text-secondary [animation-delay:60ms]">
        GPU rendering, real shell sessions and a quiet workspace interface. Written in
        Rust, with no webview.
      </p>
      <div className="mt-8 flex animate-rise flex-wrap items-center justify-center gap-2.5 [animation-delay:120ms]">
        <a
          href="/download"
          className="flex h-10 items-center gap-2 rounded-pane bg-accent pr-4 pl-[18px] text-[14px] font-medium text-on-accent transition-[filter] duration-100 hover:brightness-110 active:brightness-90"
        >
          Download Neptune
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
