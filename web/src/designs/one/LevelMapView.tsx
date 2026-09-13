import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import type { FloorFeeling, ScoutQuest } from "../../lib/wasm/types";
import { regionForDepth } from "../../lib/region";
import { FloorMapLabel } from "./FloorMapHeader";
import { ExpandIcon, XIcon } from "../../lib/icons";
import { mapRequestJson, requestLevelMap } from "../../lib/level-map/client";
import { createMapParticleRenderer } from "../../lib/level-map/particles";
import { createLevelMapRenderer } from "../../lib/level-map/render";
import type { LevelMapRequest, MapBundle } from "../../lib/level-map/types";
import {
  constrainMapTransform,
  FIT_MAP,
  mapFitScale,
  MapGesture,
  wheelZoomFactor,
  zoomMapAt,
} from "./map-gestures";
import type { MapTransform } from "./map-gestures";
import "./level-map.css";

type LevelMapViewProps = Omit<LevelMapRequest, "branch"> & {
  feeling?: FloorFeeling;
  quest?: ScoutQuest;
  floors?: { depth: number; feeling?: FloorFeeling; quest?: ScoutQuest }[];
};
const MAP_HEIGHT = 350;

/** Profile changes remount the viewer so a pinned branch can never show an old run. */
export function LevelMapView(props: LevelMapViewProps) {
  return <MapSession key={mapRequestJson(props)} {...props} />;
}
function MapSession({ feeling, quest, floors, ...props }: LevelMapViewProps) {
  const [depth, setDepth] = useState(props.depth);
  const availableFloors = floors ?? [{ depth: props.depth, feeling, quest }];
  const floorIndex = availableFloors.findIndex((floor) => floor.depth === depth);
  const currentFloor = availableFloors[floorIndex];
  const swipeStart = useRef<{ x: number; y: number } | undefined>(undefined);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [expanded, setExpanded] = useState(false);
  useEffect(() => {
    if (!expanded) return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    dialog.showModal();
    return () => {
      dialog.close();
      document.body.style.overflow = previousOverflow;
    };
  }, [expanded]);
  const [branch, setBranch] = useState(0);
  const [loaded, setLoaded] = useState<{ key: string; bundle: MapBundle }>();
  const [parent, setParent] = useState<MapBundle>();
  const [error, setError] = useState<string>();
  const [retry, setRetry] = useState(0);
  const [secrets, setSecrets] = useState(false);
  const changeFloor = (nextDepth: number) => {
    setDepth(nextDepth);
    setBranch(0);
    setParent(undefined);
    setError(undefined);
  };
  const navigate = (delta: number) => {
    const next = availableFloors[floorIndex + delta];
    if (next) {
      // The focused canvas is replaced on floor changes. Keep focus on the
      // persistent dialog so subsequent shortcuts stay inside the modal.
      dialogRef.current?.focus({ preventScroll: true });
      changeFloor(next.depth);
    }
  };
  const close = () => {
    setExpanded(false);
    if (depth !== props.depth) changeFloor(props.depth);
  };
  const request: LevelMapRequest = { ...props, depth, branch };
  const requestKey = mapRequestJson(request);
  useEffect(() => {
    let active = true;
    setError(undefined);
    void requestLevelMap(request).then(
      (next) => {
        if (!active) return;
        setLoaded({ key: requestKey, bundle: next });
        if (branch === 0) setParent(next);
      },
      (reason: unknown) => {
        if (active) setError(reason instanceof Error ? reason.message : String(reason));
      },
    );
    return () => {
      active = false;
    };
    // requestKey contains the entire canonical profile, independent of array identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestKey, retry]);
  const bundle = loaded?.key === requestKey ? loaded.bundle : undefined;
  const branches = parent?.map.branches ?? [];
  const secretCount = bundle
    ? bundle.map.secretRooms.length + bundle.map.secretDoors.length + bundle.map.secretTraps.length
    : 0;
  const title =
    branch === 0
      ? `Floor ${depth} layout`
      : bundle?.map.kind === "imp_vault"
        ? "Imp Vault"
        : "Blacksmith Mine";
  const toolbar = (
    <div className="d1-map-toolbar">
      {branches.length > 0 && (
        <div className="d1-map-branches" role="group" aria-label="Level area">
          <button type="button" aria-pressed={branch === 0} onClick={() => setBranch(0)}>
            Main
          </button>
          {branches.map((entry) => (
            <button
              type="button"
              key={entry.branch}
              aria-pressed={branch === entry.branch}
              onClick={() => setBranch(entry.branch)}
            >
              {entry.kind === "imp_vault" ? "Imp Vault" : "Blacksmith Mine"}
            </button>
          ))}
        </div>
      )}
      <button
        type="button"
        className="d1-map-secrets"
        aria-pressed={secrets}
        disabled={secretCount === 0}
        onClick={() => setSecrets((value) => !value)}
        title={
          bundle && secretCount === 0
            ? "No secrets on this map"
            : secrets
              ? "Hide secret rooms, doors and traps"
              : "Reveal secret rooms, doors and traps"
        }
      >
        <svg
          aria-hidden="true"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          {secrets ? <path d="M20 6 9 17l-5-5" /> : <path d="M18 6 6 18M6 6l12 12" />}
        </svg>
        Secrets
      </button>
    </div>
  );
  const mapContent = error ? (
    <div className="d1-map-message" role="alert">
      <p>Couldn’t load this map.</p>
      <span>{error}</span>
      <button type="button" className="d1-btn" onClick={() => setRetry((value) => value + 1)}>
        Try again
      </button>
    </div>
  ) : bundle ? (
    <MapCanvas key={requestKey} bundle={bundle} label={title} secrets={secrets} />
  ) : (
    <div className="d1-map-message" role="status">
      <span className="d1-map-loading-dot" />
      <p>Charting {branch === 0 ? `floor ${depth}` : "the quest level"}…</p>
    </div>
  );
  return (
    <div className="d1-level-map-view">
      {toolbar}
      <div className="d1-map-stage">
        {mapContent}
        <button
          type="button"
          className="d1-map-expand"
          aria-haspopup="dialog"
          aria-expanded={expanded}
          onClick={() => setExpanded(true)}
        >
          <ExpandIcon size={14} />
          Expand
        </button>
      </div>
      <dialog
        ref={dialogRef}
        className="d1-map-dialog"
        tabIndex={-1}
        aria-label={`Floor ${depth} expanded map`}
        style={{ "--region": regionForDepth(depth).color } as CSSProperties}
        onKeyDownCapture={(event) => {
          if (event.ctrlKey || event.metaKey || event.altKey) return;
          const key = event.key.toLowerCase();
          const delta =
            key === "j" || event.code === "KeyJ"
              ? 1
              : key === "k" || event.code === "KeyK"
                ? -1
                : 0;
          if (delta) {
            event.preventDefault();
            event.stopPropagation();
            navigate(delta);
          }
        }}
        onKeyDown={(event) => event.stopPropagation()}
        onTouchStart={(event) => {
          event.stopPropagation();
          const touch = event.touches[0];
          const zoomed = (event.target as HTMLElement).closest(".d1-map-zoomed");
          swipeStart.current =
            touch && event.touches.length === 1 && !zoomed
              ? { x: touch.clientX, y: touch.clientY }
              : undefined;
        }}
        onTouchCancel={() => {
          swipeStart.current = undefined;
        }}
        onTouchEnd={(event) => {
          event.stopPropagation();
          const start = swipeStart.current;
          swipeStart.current = undefined;
          const touch = event.changedTouches[0];
          if (!start || !touch) return;
          const dx = touch.clientX - start.x;
          const dy = touch.clientY - start.y;
          if (Math.abs(dx) >= 60 && Math.abs(dx) >= 1.5 * Math.abs(dy)) navigate(dx < 0 ? 1 : -1);
        }}
        onCancel={(event) => {
          event.preventDefault();
          close();
        }}
        onClose={close}
        onClick={(event) => {
          if (event.target === event.currentTarget) close();
        }}
      >
        {expanded && (
          <div className="d1-map-dialog-content">
            <header className="d1-floor-head d1-map-dialog-header">
              <nav className="d1-scout-nav d1-map-floor-nav" aria-label="Floor navigation">
                <div className="d1-map-floor-identity" aria-live="polite">
                  <FloorMapLabel
                    depth={depth}
                    feeling={currentFloor?.feeling}
                    quest={currentFloor?.quest}
                  />
                </div>
                <div className="d1-scout-nav-tools">
                  <div className="d1-scout-nav-hints" aria-hidden="true">
                    <span className="d1-scout-nav-hint d1-scout-nav-hint-keys">
                      <kbd className="d1-keycap">J</kbd>
                      <span>next</span>
                      <kbd className="d1-keycap">K</kbd>
                      <span>prev</span>
                    </span>
                    <span className="d1-scout-nav-hint d1-scout-nav-hint-swipe">
                      swipe to browse
                    </span>
                  </div>
                </div>
              </nav>
              <button
                type="button"
                className="d1-map-expand d1-map-close"
                onClick={close}
                autoFocus
              >
                <XIcon size={14} />
                Close
              </button>
            </header>
            {toolbar}
            {mapContent}
          </div>
        )}
      </dialog>
    </div>
  );
}
function MapCanvas({
  bundle,
  label,
  secrets,
}: {
  bundle: MapBundle;
  label: string;
  secrets: boolean;
}) {
  const { map } = bundle;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const particleRef = useRef<HTMLCanvasElement>(null);
  const densityRef = useRef(1);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 300, height: MAP_HEIGHT });
  const [transform, setTransform] = useState(FIT_MAP);
  const [pointerFocus, setPointerFocus] = useState(false);
  const transformRef = useRef(transform);
  const gestures = useRef(new MapGesture());
  const visibleRef = useRef(true);
  const resumeRef = useRef<() => void>(() => {});
  const widthPx = map.width * map.scene.tileSize,
    heightPx = map.height * map.scene.tileSize;
  const geometry = useMemo(
    () => ({ ...size, mapWidth: widthPx, mapHeight: heightPx }),
    [size, widthPx, heightPx],
  );
  const scale = mapFitScale(geometry) * transform.zoom;
  densityRef.current = Math.max(1, Math.min(4, scale * (window.devicePixelRatio || 1)));
  const applyTransform = useCallback((next: MapTransform) => {
    // Pointer and wheel events may arrive before React commits a render.
    transformRef.current = next;
    setTransform(next);
  }, []);
  useEffect(() => {
    const next = constrainMapTransform(transformRef.current, geometry);
    applyTransform(next);
    gestures.current.rebase(next);
  }, [applyTransform, geometry]);
  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const resize = new ResizeObserver(([entry]) => {
      setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    resize.observe(viewport);
    const intersection = new IntersectionObserver(([entry]) => {
      visibleRef.current = entry.isIntersecting;
      resumeRef.current();
    });
    intersection.observe(viewport);
    return () => {
      resize.disconnect();
      intersection.disconnect();
    };
  }, []);
  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      event.stopPropagation();
      const bounds = viewport.getBoundingClientRect();
      const current = transformRef.current;
      const next = zoomMapAt(
        current,
        current.zoom * wheelZoomFactor(event.deltaY, event.deltaMode, event.ctrlKey),
        {
          x: event.clientX - bounds.left - bounds.width / 2,
          y: event.clientY - bounds.top - bounds.height / 2,
        },
        geometry,
      );
      applyTransform(next);
      gestures.current.rebase(next);
    };
    // A native non-passive listener consumes trackpad pinch and wheel events
    // without scrolling the scout or zooming the entire browser page.
    viewport.addEventListener("wheel", wheel, { passive: false });
    return () => viewport.removeEventListener("wheel", wheel);
  }, [applyTransform, geometry]);
  useEffect(() => {
    const context = canvasRef.current?.getContext("2d");
    if (!context) return;
    const renderer = createLevelMapRenderer(context, bundle, secrets);
    const particleCanvas = particleRef.current!;
    const particles = createMapParticleRenderer(
      particleCanvas.getContext("2d")!,
      canvasRef.current!,
      bundle,
      secrets,
    );
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    const animated = renderer.animated || particles.animated;
    let frame = 0,
      last = -Infinity;
    const start = performance.now();
    const tick = (time: number) => {
      const elapsed = motion.matches ? 0 : time - start;
      if (time - last >= 50 || motion.matches) {
        renderer.draw(elapsed);
        last = time;
      }
      const density = densityRef.current;
      const width = Math.round(widthPx * density),
        height = Math.round(heightPx * density);
      if (particleCanvas.width !== width || particleCanvas.height !== height) {
        particleCanvas.width = width;
        particleCanvas.height = height;
      }
      particles.draw(elapsed);
      if (animated && !motion.matches && visibleRef.current && !document.hidden)
        frame = requestAnimationFrame(tick);
    };
    const resume = () => {
      cancelAnimationFrame(frame);
      if (visibleRef.current && !document.hidden) frame = requestAnimationFrame(tick);
    };
    resumeRef.current = resume;
    tick(start);
    document.addEventListener("visibilitychange", resume);
    motion.addEventListener("change", resume);
    return () => {
      resumeRef.current = () => {};
      cancelAnimationFrame(frame);
      document.removeEventListener("visibilitychange", resume);
      motion.removeEventListener("change", resume);
    };
  }, [bundle, secrets, widthPx, heightPx]);
  const pointerPoint = (event: ReactPointerEvent<HTMLDivElement>) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: event.clientX - bounds.left - bounds.width / 2,
      y: event.clientY - bounds.top - bounds.height / 2,
    };
  };
  const pointDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    // Programmatic focus can retain the browser’s prior keyboard-focus styling.
    setPointerFocus(true);
    event.currentTarget.focus({ preventScroll: true });
    event.currentTarget.setPointerCapture(event.pointerId);
    gestures.current.down(event.pointerId, pointerPoint(event), transformRef.current);
  };
  const pointMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const next = gestures.current.move(event.pointerId, pointerPoint(event), geometry);
    if (next) {
      event.stopPropagation();
      applyTransform(next);
    }
  };
  const pointUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    gestures.current.up(event.pointerId, transformRef.current);
  };
  const canvasStyle: CSSProperties = {
    width: widthPx * scale,
    height: heightPx * scale,
    transform: `translate(calc(-50% + ${transform.x}px), calc(-50% + ${transform.y}px))`,
  };
  return (
    <div
      ref={viewportRef}
      className={`d1-map-viewport ${transform.zoom > 1 ? "d1-map-zoomed" : ""}`}
      tabIndex={0}
      data-pointer-focus={pointerFocus || undefined}
      onBlur={() => setPointerFocus(false)}
      role="region"
      aria-label={label}
      onPointerDown={pointDown}
      onPointerMove={pointMove}
      onPointerUp={pointUp}
      onPointerCancel={pointUp}
      onLostPointerCapture={pointUp}
      onKeyDown={(event) => {
        setPointerFocus(false);
        if (event.ctrlKey || event.metaKey || event.altKey) return;
        if (
          ["+", "=", "-", "0", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
            event.key,
          )
        ) {
          event.stopPropagation();
          event.preventDefault();
          const current = transformRef.current;
          let next = current;
          if (event.key === "0") next = FIT_MAP;
          else if (event.key === "+" || event.key === "=" || event.key === "-")
            next = zoomMapAt(
              current,
              current.zoom * (event.key === "-" ? 1 / 1.5 : 1.5),
              { x: 0, y: 0 },
              geometry,
            );
          else
            next = constrainMapTransform(
              {
                ...current,
                x:
                  current.x +
                  (event.key === "ArrowLeft" ? 24 : event.key === "ArrowRight" ? -24 : 0),
                y: current.y + (event.key === "ArrowUp" ? 24 : event.key === "ArrowDown" ? -24 : 0),
              },
              geometry,
            );
          applyTransform(next);
          gestures.current.rebase(next);
        }
      }}
    >
      <canvas
        ref={canvasRef}
        width={widthPx}
        height={heightPx}
        style={canvasStyle}
        role="img"
        aria-label={label}
      />
      <canvas
        ref={particleRef}
        style={{ ...canvasStyle, pointerEvents: "none" }}
        aria-hidden="true"
      />
    </div>
  );
}
