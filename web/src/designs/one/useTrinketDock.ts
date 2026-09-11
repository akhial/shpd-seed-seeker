import { useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { ScoutResult } from "../../lib/wasm/types";

/** Scrub the compact controls as the original offers pass behind a floor header. */
export function useTrinketDock(
  result: ScoutResult | undefined,
  scrollRef: RefObject<HTMLDivElement | null>,
  summaryRef: RefObject<HTMLDivElement | null>,
) {
  const offersRef = useRef<HTMLOListElement>(null);
  const dockRef = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const anchor = useRef<{ seed: string; depth: string; offset: number } | undefined>(undefined);

  const rememberFloor = () => {
    const scroll = scrollRef.current;
    if (!scroll || !result) return;
    const top = contentTop(scroll, summaryRef.current);
    const floor = [...scroll.querySelectorAll<HTMLElement>(".d1-floor")].find(
      (element) => element.getBoundingClientRect().bottom > top,
    );
    if (floor) {
      anchor.current = {
        seed: result.seed.code,
        depth: floor.dataset.depth!,
        offset: floor.getBoundingClientRect().top - top,
      };
    }
  };

  useLayoutEffect(() => {
    const scroll = scrollRef.current;
    const offers = offersRef.current;
    const dock = dockRef.current;
    const saved = anchor.current;
    anchor.current = undefined;
    if (!scroll || !offers || !dock) {
      dock?.style.setProperty("--trinket-reveal", "0");
      setVisible(false);
      return;
    }

    if (saved && saved.seed === result?.seed.code) {
      const floor = [...scroll.querySelectorAll<HTMLElement>(".d1-floor")].find(
        (element) => element.dataset.depth === saved.depth,
      );
      if (floor) {
        const headerHeight =
          floor.querySelector(".d1-floor-head")?.getBoundingClientRect().height ?? 0;
        const offset = Math.max(saved.offset, headerHeight - floor.getBoundingClientRect().height);
        const delta =
          floor.getBoundingClientRect().top - contentTop(scroll, summaryRef.current) - offset;
        if (usesPaneScroll(scroll)) scroll.scrollTop += delta;
        else window.scrollBy(0, delta);
      }
    }

    let frame: number | undefined;
    const update = () => {
      const bounds = offers.getBoundingClientRect();
      const header = offers.closest(".d1-floor")?.querySelector(".d1-floor-head");
      const edge =
        contentTop(scroll, summaryRef.current) + (header?.getBoundingClientRect().height ?? 0);
      const progress =
        scroll.clientHeight > 0 && bounds.height > 0
          ? Math.max(0, Math.min(1, (edge - bounds.top) / bounds.height))
          : 0;
      dock.style.setProperty("--trinket-reveal", String(progress));
      setVisible(progress > 0);
    };
    const schedule = () => {
      if (frame !== undefined) return;
      frame = requestAnimationFrame(() => {
        frame = undefined;
        update();
      });
    };
    update();
    scroll.addEventListener("scroll", schedule, { passive: true });
    // Mobile Scout scrolls the page; desktop scrolls the pane. Listen to both
    // so resizing or opening the mobile Scout tab never leaves a stale host.
    window.addEventListener("scroll", schedule, { passive: true });
    window.addEventListener("resize", schedule);
    const observer = new ResizeObserver(schedule);
    observer.observe(scroll);
    observer.observe(offers);
    observer.observe(dock);
    if (summaryRef.current) observer.observe(summaryRef.current);
    if (scroll.firstElementChild) observer.observe(scroll.firstElementChild);
    return () => {
      scroll.removeEventListener("scroll", schedule);
      window.removeEventListener("scroll", schedule);
      window.removeEventListener("resize", schedule);
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [result, scrollRef, summaryRef]);

  return { offersRef, dockRef, visible, rememberFloor };
}

function usesPaneScroll(scroll: HTMLElement) {
  return ["auto", "scroll"].includes(getComputedStyle(scroll).overflowY);
}

/** The summary and floor header occupy the top of either scroll viewport. */
function contentTop(scroll: HTMLElement, summary: HTMLElement | null) {
  const viewportTop = usesPaneScroll(scroll) ? scroll.getBoundingClientRect().top : 0;
  return viewportTop + (summary?.getBoundingClientRect().height ?? 0);
}
