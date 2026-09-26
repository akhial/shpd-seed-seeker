import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import type { FloorFeeling, ScoutQuest, TrinketOffer } from "../../engine/types";
import { regionForDepth } from "../../shared/game/region";
import { FloorMapLabel } from "../scout/FloorMapHeader";
import { TrinketShortcuts } from "../scout/trinkets/TrinketShortcuts";
import { ExpandIcon, XIcon } from "../../shared/ui/icons";
import { mapRequestJson, requestLevelMap } from "./client";
import { createMapFrameRenderer } from "./rendering/frame-renderer";
import type { LevelMapRequest, MapBundle } from "./types";
import {
  constrainMapTransform,
  FIT_MAP,
  mapFitScale,
  MapGesture,
  wheelZoomFactor,
  zoomMapAt,
} from "./map-gestures";
import type { MapTransform } from "./map-gestures";
import { itemAtPoint, itemBounds } from "./item-inspection";
import { MapItemTooltip } from "./MapItemTooltip";
import "./level-map.css";

type LevelMapViewProps = Omit<LevelMapRequest, "branch"> & {
  feeling?: FloorFeeling;
  quest?: ScoutQuest;
  floors?: { depth: number; feeling?: FloorFeeling; quest?: ScoutQuest }[];
  trinketOffers?: readonly TrinketOffer[];
  onTrinketChange?: (trinket: string) => void;
  changingTrinket?: boolean;
};
const MAP_HEIGHT = 350;

/** New runs remount; trinket swaps preserve the expanded dialog and current floor. */
export function LevelMapView(props: LevelMapViewProps) {
  return <MapSession key={mapRequestJson({ ...props, selectedTrinket: "none" })} {...props} />;
}
function MapSession({
  feeling,
  quest,
  floors,
  trinketOffers,
  onTrinketChange,
  changingTrinket,
  ...props
}: LevelMapViewProps) {
  const [depth, setDepth] = useState(props.depth);
  const availableFloors = floors ?? [{ depth: props.depth, feeling, quest }];
  const floorIndex = availableFloors.findIndex((floor) => floor.depth === depth);
  const currentFloor = availableFloors[floorIndex];
  const swipeStart = useRef<{ x: number; y: number } | undefined>(undefined);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [selection, setSelection] = useState<{ branch: number; secrets: boolean }>();
  const expanded = selection !== undefined;
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
  const navigate = (delta: number) => {
    const next = availableFloors[floorIndex + delta];
    if (next) {
      dialogRef.current?.focus({ preventScroll: true });
      setDepth(next.depth);
    }
  };
  const close = () => setSelection(undefined);
  return (
    <div className="d1-level-map-view">
      <MapPanel
        {...props}
        animated={!expanded}
        expanded={expanded}
        onExpand={(branch, secrets) => {
          setDepth(props.depth);
          setSelection({ branch, secrets });
        }}
      />
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
                {onTrinketChange && trinketOffers && trinketOffers.length > 0 && (
                  <TrinketShortcuts
                    className="d1-map-trinkets"
                    offers={trinketOffers}
                    selectedTrinket={props.selectedTrinket}
                    onSelect={(trinket) => {
                      // Loading disables the clicked button. Keep keyboard focus
                      // inside the dialog so J/K navigation continues to work.
                      dialogRef.current?.focus({ preventScroll: true });
                      onTrinketChange(trinket);
                    }}
                    disabled={changingTrinket}
                  />
                )}
                <div className="d1-map-header-controls">
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
                  <button
                    type="button"
                    className="d1-map-expand d1-map-close"
                    onClick={close}
                    autoFocus
                  >
                    <XIcon size={14} />
                    Close
                  </button>
                </div>
              </nav>
            </header>
            <MapPanel
              {...props}
              depth={depth}
              initialBranch={selection.branch}
              initialSecrets={selection.secrets}
            />
          </div>
        )}
      </dialog>
    </div>
  );
}
/** Each inline/expanded panel owns its location, load lifecycle and viewport. */
function MapPanel({
  initialBranch = 0,
  initialSecrets = false,
  animated = true,
  expanded = false,
  onExpand,
  ...props
}: Omit<LevelMapRequest, "branch"> & {
  initialBranch?: number;
  initialSecrets?: boolean;
  animated?: boolean;
  expanded?: boolean;
  onExpand?: (branch: number, secrets: boolean) => void;
}) {
  const { depth } = props;
  const profileKey = mapRequestJson(props);
  const locationKey = mapRequestJson({ ...props, selectedTrinket: "none" });
  const [branchSelection, selectBranch] = useState({ key: locationKey, branch: initialBranch });
  const branch = branchSelection.key === locationKey ? branchSelection.branch : 0;
  const setBranch = (next: number) => selectBranch({ key: locationKey, branch: next });
  const [loaded, setLoaded] = useState<{ key: string; bundle: MapBundle }>();
  const [parent, setParent] = useState<{ key: string; location: string; bundle: MapBundle }>();
  const [error, setError] = useState<string>();
  const [retry, setRetry] = useState(0);
  const [secrets, setSecrets] = useState(initialSecrets);
  const request: LevelMapRequest = { ...props, depth, branch };
  const requestKey = mapRequestJson(request);
  useEffect(() => {
    let active = true;
    setError(undefined);
    void (async () => {
      try {
        let next: MapBundle;
        if (parent?.key !== profileKey) {
          const main = await requestLevelMap({ ...request, branch: 0 });
          if (!active) return;
          setParent({ key: profileKey, location: locationKey, bundle: main });
          if (branch !== 0 && !main.map.branches.some((area) => area.branch === branch)) {
            setBranch(0);
            return;
          }
          next = branch === 0 ? main : await requestLevelMap(request);
        } else {
          next = await requestLevelMap(request);
        }
        if (active) setLoaded({ key: requestKey, bundle: next });
      } catch (reason) {
        if (active) setError(reason instanceof Error ? reason.message : String(reason));
      }
    })();
    return () => {
      active = false;
    };
    // requestKey contains the entire canonical profile, independent of array identity.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestKey, retry]);
  const bundle = loaded?.key === requestKey ? loaded.bundle : undefined;
  const branches = parent?.location === locationKey ? parent.bundle.map.branches : [];
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
  ) : !bundle ? (
    <div className="d1-map-message" role="status">
      <span className="d1-map-loading-dot" />
      <p>Charting {branch === 0 ? `floor ${depth}` : "the quest level"}…</p>
    </div>
  ) : null;
  return (
    <>
      {toolbar}
      <div className="d1-map-stage">
        <MapCanvas
          key={mapRequestJson({ ...request, selectedTrinket: "none" })}
          bundle={loaded?.bundle}
          ready={bundle !== undefined}
          animated={animated}
          label={title}
          secrets={secrets}
        />
        {mapContent}
        {onExpand && (
          <button
            type="button"
            className="d1-map-expand"
            aria-haspopup="dialog"
            aria-expanded={expanded}
            onClick={() => onExpand(branch, secrets)}
          >
            <ExpandIcon size={14} />
            Expand
          </button>
        )}
      </div>
    </>
  );
}

function MapCanvas({
  bundle,
  label,
  secrets,
  ready,
  animated: active,
}: {
  bundle?: MapBundle;
  ready: boolean;
  animated: boolean;
  label: string;
  secrets: boolean;
}) {
  const map = bundle?.map;
  const tooltipId = useId();
  const [inspected, setInspected] = useState<number>();
  const hideTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const lastHovered = useRef<number | undefined>(undefined);
  const dismissed = useRef<number | undefined>(undefined);
  const cancelHide = () => {
    clearTimeout(hideTimer.current);
    hideTimer.current = undefined;
  };
  useEffect(() => () => clearTimeout(hideTimer.current), []);
  const pointerDown = useRef<{ id: number; x: number; y: number; moved: boolean } | undefined>(
    undefined,
  );
  const tip =
    ready && active
      ? map?.itemTooltips?.find((item) => item.cell === inspected && (secrets || !item.hidden))
      : undefined;
  useEffect(() => {
    clearTimeout(hideTimer.current);
    lastHovered.current = undefined;
    dismissed.current = undefined;
    setInspected(undefined);
  }, [bundle, ready, active, secrets]);
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
  const widthPx = map ? map.width * map.scene.tileSize : 1,
    heightPx = map ? map.height * map.scene.tileSize : 1;
  const geometry = useMemo(
    () => ({ ...size, mapWidth: widthPx, mapHeight: heightPx }),
    [size, widthPx, heightPx],
  );
  const scale = mapFitScale(geometry) * transform.zoom;
  densityRef.current = Math.max(1, Math.min(4, scale * (window.devicePixelRatio || 1)));
  const applyTransform = useCallback((next: MapTransform) => {
    // Pointer and wheel events may arrive before React commits a render.
    setInspected(undefined);
    transformRef.current = next;
    setTransform(next);
  }, []);
  useEffect(() => {
    if (!ready) return;
    const next = constrainMapTransform(transformRef.current, geometry);
    applyTransform(next);
    gestures.current.rebase(next);
  }, [applyTransform, geometry, ready]);
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
      if (event.target instanceof Element && event.target.closest(".d1-map-item-tooltip")) return;
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
    const particleContext = particleRef.current?.getContext("2d");
    if (!context || !particleContext) return;
    if (!ready || !bundle) {
      context.clearRect(0, 0, widthPx, heightPx);
      particleContext.clearRect(0, 0, particleContext.canvas.width, particleContext.canvas.height);
      return;
    }
    const renderer = createMapFrameRenderer(context, particleContext, bundle, secrets);
    const motion = window.matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0,
      last = -Infinity;
    let stopped = false;
    let drawing = false;
    let redraw = true;
    const start = performance.now();
    const visible = () => visibleRef.current && !document.hidden;
    const schedule = () => {
      if (!stopped && !frame && !drawing && visible()) frame = requestAnimationFrame(tick);
    };
    const tick = (time: number) => {
      frame = 0;
      if (stopped || drawing || !visible()) return;
      const advanceSprites = time - last >= 50 || motion.matches;
      if (advanceSprites) last = time;
      redraw = false;
      drawing = true;
      void renderer
        .draw(motion.matches ? 0 : time - start, advanceSprites, densityRef.current)
        .then((animated) => {
          drawing = false;
          if (redraw || (active && animated && !motion.matches)) schedule();
        });
    };
    const resume = () => {
      cancelAnimationFrame(frame);
      frame = 0;
      redraw = true;
      schedule();
    };
    resumeRef.current = resume;
    resume();
    document.addEventListener("visibilitychange", resume);
    motion.addEventListener("change", resume);
    return () => {
      resumeRef.current = () => {};
      stopped = true;
      cancelAnimationFrame(frame);
      renderer.dispose();
      document.removeEventListener("visibilitychange", resume);
      motion.removeEventListener("change", resume);
    };
  }, [bundle, ready, active, secrets, widthPx, heightPx]);
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
    setInspected(undefined);
    if (pointerDown.current) pointerDown.current.moved = true;
    else
      pointerDown.current = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
        moved: false,
      };
    event.currentTarget.setPointerCapture(event.pointerId);
    gestures.current.down(event.pointerId, pointerPoint(event), transformRef.current);
  };
  const inspect = (event: ReactPointerEvent<HTMLDivElement>) => {
    const cell =
      ready && map
        ? itemAtPoint(map, pointerPoint(event), geometry, transformRef.current, secrets)?.cell
        : undefined;
    if (cell !== lastHovered.current) dismissed.current = undefined;
    lastHovered.current = cell;
    cancelHide();
    if (cell === undefined) hideTimer.current = setTimeout(() => setInspected(undefined), 180);
    else if (dismissed.current !== cell) setInspected(cell);
  };
  const pointMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const down = pointerDown.current;
    if (down && Math.hypot(event.clientX - down.x, event.clientY - down.y) > 6) down.moved = true;
    if (!down && event.pointerType !== "touch") inspect(event);
    const next = gestures.current.move(event.pointerId, pointerPoint(event), geometry);
    if (next) {
      event.stopPropagation();
      applyTransform(next);
    }
  };
  const pointUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    const down = pointerDown.current;
    if (event.type === "pointerup" && down?.id === event.pointerId && !down.moved) inspect(event);
    if (down?.id === event.pointerId) pointerDown.current = undefined;
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
      onBlur={(event) => {
        setPointerFocus(false);
        if (!event.currentTarget.contains(event.relatedTarget)) setInspected(undefined);
      }}
      onPointerLeave={() => setInspected(undefined)}
      role="region"
      aria-label={label}
      onPointerDown={pointDown}
      onPointerMove={pointMove}
      onPointerUp={pointUp}
      onPointerCancel={pointUp}
      onLostPointerCapture={pointUp}
      onKeyDown={(event) => {
        setPointerFocus(false);
        if (event.key === "Escape" && tip) {
          event.preventDefault();
          event.stopPropagation();
          dismissed.current = tip.cell;
          setInspected(undefined);
          return;
        }
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
      {ready &&
        active &&
        map?.itemTooltips
          ?.filter((item) => secrets || !item.hidden)
          .map((item) => {
            const [left, top, width, height] = itemBounds(item, map.width, map.scene.tileSize);
            return (
              <button
                key={item.cell}
                type="button"
                className="d1-map-item-target"
                aria-label={item.items.map((entry) => entry.name).join(", ")}
                aria-describedby={tip?.cell === item.cell ? tooltipId : undefined}
                onFocus={() => {
                  cancelHide();
                  setInspected(item.cell);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    setInspected(item.cell);
                  }
                }}
                style={{
                  width: width * scale,
                  height: height * scale,
                  left: size.width / 2 + transform.x - (widthPx * scale) / 2 + left * scale,
                  top: size.height / 2 + transform.y - (heightPx * scale) / 2 + top * scale,
                }}
              />
            );
          })}
      {tip && map && (
        <MapItemTooltip
          tip={tip}
          onPointerEnter={cancelHide}
          id={tooltipId}
          width={size.width}
          height={size.height}
          x={
            size.width / 2 +
            transform.x +
            (((tip.cell % map.width) + 0.5) * map.scene.tileSize - widthPx / 2) * scale
          }
          y={
            size.height / 2 +
            transform.y +
            ((Math.floor(tip.cell / map.width) + 0.5) * map.scene.tileSize - heightPx / 2) * scale
          }
        />
      )}
      <canvas
        ref={particleRef}
        style={{ ...canvasStyle, pointerEvents: "none" }}
        aria-hidden="true"
      />
    </div>
  );
}
