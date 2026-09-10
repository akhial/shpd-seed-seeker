import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { XIcon } from "../../lib/icons";
import { prefetchLevelMap } from "../../lib/level-map/client";
import { regionForDepth } from "../../lib/region";
import { LevelMapView } from "./LevelMapView";
import { lensPosition } from "./map-lens";
import "./map-lens.css";

type MapProfile = {
  seed: string;
  depth: number;
  challenges: readonly string[];
  selectedTrinket?: string | null;
};

function MapIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path
        d="m1.5 4 5-2 5 2 5-2v12l-5 2-5-2-5 2V4ZM6.5 2v12m5-10v12"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function FloorMapButton({
  seed,
  depth,
  challenges,
  selectedTrinket,
  active,
  onOpen,
}: MapProfile & {
  active: boolean;
  onOpen: (anchor: HTMLButtonElement) => void;
}) {
  const prefetchTimer = useRef<number | undefined>(undefined);
  const cancelPrefetch = () => {
    window.clearTimeout(prefetchTimer.current);
    prefetchTimer.current = undefined;
  };
  useEffect(() => () => window.clearTimeout(prefetchTimer.current), []);
  const prefetch = () => {
    if (active) return;
    cancelPrefetch();
    // Passing over several headers while scrolling should not queue whole
    // floors. The worker client also shares in-flight and completed requests.
    prefetchTimer.current = window.setTimeout(() => {
      void prefetchLevelMap({ seed, depth, challenges, selectedTrinket }).catch(() => undefined);
    }, 250);
  };
  return (
    <button
      type="button"
      className="d1-floor-map-trigger"
      data-floor-map-depth={depth}
      aria-label={`View map of floor ${depth}`}
      aria-haspopup="dialog"
      aria-expanded={active}
      aria-controls={active ? "d1-scout-map-lens" : undefined}
      title={`Inspect floor ${depth} layout`}
      onPointerEnter={(event) => {
        if (event.pointerType === "mouse") prefetch();
      }}
      onPointerLeave={cancelPrefetch}
      onFocus={prefetch}
      onBlur={cancelPrefetch}
      onClick={(event) => {
        cancelPrefetch();
        onOpen(event.currentTarget);
      }}
    >
      <MapIcon />
      <span>Map</span>
    </button>
  );
}

export function ScoutMapLens({
  seed,
  depth,
  challenges,
  selectedTrinket,
  anchor,
  depths,
  onDepthChange,
  onClose,
}: MapProfile & {
  anchor: HTMLButtonElement;
  depths: readonly number[];
  onDepthChange: (depth: number) => void;
  onClose: () => void;
}) {
  const popup = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const returnAnchor = useRef(anchor);
  const [mapHeight, setMapHeight] = useState(410);
  const region = regionForDepth(depth);
  const index = depths.indexOf(depth);

  useLayoutEffect(() => {
    returnAnchor.current = anchor;
    const element = popup.current;
    if (!element) return;
    const place = () => {
      const viewportHeight = window.visualViewport?.height ?? window.innerHeight;
      const viewportWidth = window.visualViewport?.width ?? window.innerWidth;
      const pane = anchor.closest(".d1-pane-scout");
      const position = lensPosition({
        viewportWidth,
        viewportHeight,
        paneLeft: pane?.getBoundingClientRect().left ?? anchor.getBoundingClientRect().left,
        anchorTop: anchor.getBoundingClientRect().top,
        popupHeight: element.getBoundingClientRect().height,
      });
      element.style.left = `${position.left}px`;
      element.style.top = `${position.top}px`;
      element.style.width = `${position.width}px`;
      setMapHeight(Math.max(180, Math.min(410, viewportHeight - 260)));
    };
    place();
    const observer = new ResizeObserver(place);
    observer.observe(element);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    window.visualViewport?.addEventListener("resize", place);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      window.visualViewport?.removeEventListener("resize", place);
    };
  }, [anchor]);

  useLayoutEffect(() => {
    const element = popup.current;
    if (!element) return;
    element.showPopover();
    closeButton.current?.focus({ preventScroll: true });
    const dismissOutside = (event: PointerEvent) => {
      const target = event.target;
      // A second floor's trigger changes this inspector directly, so maps
      // stay a single contextual surface rather than a stack of windows.
      if (
        target instanceof Element &&
        !element.contains(target) &&
        !target.closest("[data-floor-map-depth]")
      )
        onClose();
    };
    const dismissWithEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    };
    document.addEventListener("pointerdown", dismissOutside);
    document.addEventListener("keydown", dismissWithEscape, true);
    return () => {
      document.removeEventListener("pointerdown", dismissOutside);
      document.removeEventListener("keydown", dismissWithEscape, true);
      element.hidePopover();
      if (returnAnchor.current.isConnected) returnAnchor.current.focus({ preventScroll: true });
    };
  }, [onClose]);

  return createPortal(
    <div
      id="d1-scout-map-lens"
      className="d1-map-lens"
      ref={popup}
      popover="manual"
      role="dialog"
      data-scout-map=""
      aria-modal="false"
      aria-labelledby="d1-map-lens-title"
      style={{ ["--region" as string]: region.color }}
      onTouchStart={(event) => event.stopPropagation()}
      onTouchEnd={(event) => event.stopPropagation()}
    >
      <header className="d1-map-lens-head">
        <div className="d1-map-lens-heading">
          <span className="d1-map-lens-eyebrow">
            Level map <span aria-hidden="true">·</span> <span className="d1-mono">{seed}</span>
          </span>
          <h3 id="d1-map-lens-title" aria-live="polite">
            Floor {depth} <span aria-hidden="true">·</span> <span>{region.name}</span>
          </h3>
        </div>
        <nav className="d1-map-lens-navigation" aria-label="Map floor navigation">
          <button
            type="button"
            className="d1-map-lens-icon-button"
            disabled={index <= 0}
            aria-label="Previous floor map"
            title="Previous floor map"
            onClick={() => onDepthChange(depths[index - 1])}
          >
            ‹
          </button>
          <button
            type="button"
            className="d1-map-lens-icon-button"
            disabled={index < 0 || index >= depths.length - 1}
            aria-label="Next floor map"
            title="Next floor map"
            onClick={() => onDepthChange(depths[index + 1])}
          >
            ›
          </button>
        </nav>
        <button
          type="button"
          className="d1-map-lens-icon-button d1-map-lens-close"
          aria-label="Close level map"
          title="Close map (Escape)"
          ref={closeButton}
          onClick={onClose}
        >
          <XIcon size={16} />
        </button>
      </header>
      <LevelMapView
        seed={seed}
        depth={depth}
        challenges={challenges}
        selectedTrinket={selectedTrinket}
        height={mapHeight}
      />
    </div>,
    document.body,
  );
}
