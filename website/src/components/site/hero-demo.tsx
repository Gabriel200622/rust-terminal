"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { Icon } from "../icons";
import { Store } from "../neptune/model";
import { BASE, CHAPTERS, Tour } from "../neptune/tour";
import { NeptuneWindow } from "../neptune/window";
import { useReducedMotion } from "../prefs";

interface Playing {
  index: number;
  duration: number;
  /** Restarts the progress line when a chapter is replayed. */
  run: number;
}

/**
 * The hero: Neptune's window, playing a short tour of what it does. Choosing a
 * chapter jumps to it; using the window hands it to the visitor.
 */
export function HeroDemo() {
  const [store] = useState(() => new Store(BASE));
  const [playing, setPlaying] = useState<Playing | null>(null);
  const [selected, setSelected] = useState(0);
  // The tour waits while the window is off screen or the tab is hidden.
  const [waiting, setWaiting] = useState(false);
  // A visitor used the window, so the caption stops narrating the tour.
  const [driving, setDriving] = useState(false);
  const tabs = useRef<HTMLDivElement>(null);
  const tour = useRef<Tour | null>(null);
  const frame = useRef<HTMLDivElement>(null);
  const reduced = useReducedMotion();

  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const instance = new Tour(
      store,
      {
        onChapter: (index, duration) => {
          setSelected(index);
          setDriving(false);
          // Keep the playing chapter's tab in view where the tabs scroll.
          const strip = tabs.current;
          const tab = strip?.children[index] as HTMLElement | undefined;
          if (strip && tab) {
            const left = tab.offsetLeft - strip.offsetLeft;
            if (left < strip.scrollLeft || left + tab.offsetWidth > strip.scrollLeft + strip.clientWidth) {
              strip.scrollTo({ left: left - 24, behavior: "smooth" });
            }
          }
          setPlaying((current) => ({ index, duration, run: (current?.run ?? 0) + 1 }));
        },
      },
      () => element.clientWidth < 760,
    );
    tour.current = instance;
    if (reduced) {
      // Without motion, show a finished layout instead of a performance.
      void instance.show(1).then(() => {
        setSelected(1);
        setPlaying(null);
      });
      return () => instance.stop();
    }

    let onScreen = true;
    const update = () => {
      const paused = !onScreen || document.hidden;
      instance.setPaused(paused);
      setWaiting(paused);
    };
    const observer = new IntersectionObserver(
      ([entry]) => {
        onScreen = entry.isIntersecting;
        update();
      },
      { threshold: 0.25 },
    );
    observer.observe(element);
    document.addEventListener("visibilitychange", update);
    instance.play(0);
    return () => {
      instance.stop();
      observer.disconnect();
      document.removeEventListener("visibilitychange", update);
    };
  }, [store, reduced]);

  const takeover = useCallback(() => {
    if (!tour.current?.playing) return;
    tour.current.stop();
    setPlaying(null);
    setDriving(true);
    // Fields the tour was filling in become the visitor's to type in.
    const state = store.get();
    const overlay = state.overlay;
    if (overlay.kind === "palette" || overlay.kind === "ssh") {
      store.dispatch({ type: "overlay", overlay: { ...overlay, typed: true } });
    }
    if (state.drag?.scripted) store.dispatch({ type: "drag", drag: null });
    // A command the tour was halfway through printing gives its prompt back.
    for (const pane of Object.values(state.panes)) {
      if (pane.status === "running" && !pane.prompt && !pane.busy) {
        store.dispatch({ type: "ready", pane: pane.id });
      }
    }
    if (state.search.open) {
      store.dispatch({ type: "search", open: true, query: state.search.query, typed: true });
    }
  }, [store]);

  const choose = (index: number) => {
    setSelected(index);
    if (reduced) {
      void tour.current?.show(index);
      return;
    }
    tour.current?.play(index);
  };

  const toggle = () => {
    if (playing) {
      tour.current?.stop();
      setPlaying(null);
    } else {
      tour.current?.play(selected);
    }
  };

  return (
    <div className="flex w-full flex-col items-center">
      <div className="flex max-w-full items-center gap-1">
        {!reduced && (
          <button
            type="button"
            onClick={toggle}
            aria-label={playing ? "Pause the tour" : "Play the tour"}
            className="grid size-[30px] shrink-0 cursor-pointer place-items-center rounded-control text-secondary transition-colors duration-100 hover:bg-hover hover:text-fg active:bg-pressed"
          >
            <Icon name={playing ? "pause" : "play"} size={14} />
          </button>
        )}
        <div
          ref={tabs}
          role="group"
          aria-label="What the window is showing"
          className="flex min-w-0 items-center gap-0.5 overflow-x-auto [scrollbar-width:none]"
        >
          {CHAPTERS.map((chapter, index) => {
            const active = index === selected;
            return (
              <button
                key={chapter.id}
                type="button"
                aria-pressed={active}
                onClick={() => choose(index)}
                className={`relative h-[30px] shrink-0 cursor-pointer overflow-hidden rounded-control px-3 text-[13px] font-medium transition-colors duration-100 ${
                  active ? "bg-control text-fg" : "text-muted hover:text-secondary"
                }`}
              >
                {chapter.label}
                {active && playing?.index === index && (
                  <span
                    key={playing.run}
                    className="absolute inset-x-0 bottom-0 h-[2px] origin-left bg-accent"
                    style={{
                      animation: `chapter-fill ${playing.duration}ms linear both`,
                      animationPlayState: waiting ? "paused" : "running",
                    }}
                  />
                )}
              </button>
            );
          })}
        </div>
      </div>
      <p
        key={driving ? "driving" : selected}
        className="mt-3 flex min-h-[2.9em] max-w-[34rem] animate-fade-in items-start justify-center px-2 text-center text-[14px] leading-[1.45] text-secondary sm:min-h-[1.45em] sm:max-w-none"
      >
        {driving
          ? "It is yours now. Click a pane and type help, or press play to resume."
          : CHAPTERS[selected].caption}
      </p>

      <div
        ref={frame}
        className="relative mt-6 aspect-[10/11] w-full max-[639px]:[zoom:0.82] sm:aspect-[4/3] lg:aspect-[1180/740]"
      >
        <NeptuneWindow store={store} onTakeover={takeover} touring={playing !== null} />
      </div>
    </div>
  );
}
