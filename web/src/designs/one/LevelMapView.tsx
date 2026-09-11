import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { mapRequestJson, requestLevelMap } from "../../lib/level-map/client";
import { drawLevelMap } from "../../lib/level-map/render";
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

export interface LevelMapViewProps {
  seed: string;
  depth: number;
  challenges: readonly string[];
  selectedTrinket?: string | null;
  height?: number;
  compact?: boolean;
  className?: string;
}
/** Profile changes remount the viewer so a pinned branch can never show an old run. */
export function LevelMapView(props: LevelMapViewProps) {
  return <MapSession key={mapRequestJson(props)} {...props} />;
}
function MapSession(props: LevelMapViewProps) {
  const [branch, setBranch] = useState(0);
  const [loaded, setLoaded] = useState<{ key: string; bundle: MapBundle }>();
  const [parent, setParent] = useState<MapBundle>();
  const [error, setError] = useState<string>();
  const [retry, setRetry] = useState(0);
  const [secrets, setSecrets] = useState(false);
  const request: LevelMapRequest = { ...props, branch };
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
      ? `Floor ${props.depth} layout`
      : bundle?.map.kind === "imp_vault"
        ? "Imp vault"
        : "Blacksmith mine";
  return (
    <div
      className={`d1-level-map-view ${props.compact ? "d1-level-map-compact" : ""} ${props.className ?? ""}`}
    >
      <div className={`d1-map-toolbar${branches.length > 0 ? " d1-map-toolbar-branched" : ""}`}>
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
                {entry.kind === "imp_vault" ? "Imp vault" : "Blacksmith mine"}
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
            secrets ? "Hide secret rooms, doors and traps" : "Reveal secret rooms, doors and traps"
          }
        >
          Secrets
        </button>
      </div>
      {error ? (
        <div className="d1-map-message" role="alert" style={{ minHeight: props.height ?? 300 }}>
          <p>Couldn’t load this map.</p>
          <span>{error}</span>
          <button type="button" className="d1-btn" onClick={() => setRetry((value) => value + 1)}>
            Try again
          </button>
        </div>
      ) : bundle ? (
        <MapCanvas
          key={requestKey}
          bundle={bundle}
          height={props.height ?? (props.compact ? 230 : 320)}
          label={title}
          secrets={secrets}
        />
      ) : (
        <div className="d1-map-message" role="status" style={{ minHeight: props.height ?? 300 }}>
          <span className="d1-map-loading-dot" />
          <p>Charting {branch === 0 ? `floor ${props.depth}` : "the quest level"}…</p>
        </div>
      )}
    </div>
  );
}
function MapCanvas({
  bundle,
  height,
  label,
  secrets,
}: {
  bundle: MapBundle;
  height: number;
  label: string;
  secrets: boolean;
}) {
  const { map } = bundle;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 300, height });
  const [transform, setTransform] = useState(FIT_MAP);
  const transformRef = useRef(transform);
  const gestures = useRef(new MapGesture());
  const [visible, setVisible] = useState(true);
  const widthPx = map.width * map.scene.tileSize,
    heightPx = map.height * map.scene.tileSize;
  const geometry = useMemo(
    () => ({ ...size, mapWidth: widthPx, mapHeight: heightPx }),
    [size, widthPx, heightPx],
  );
  const scale = mapFitScale(geometry) * transform.zoom;
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
    const intersection = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting));
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
    let frame = 0,
      last = -Infinity;
    const start = performance.now();
    const tick = (time: number) => {
      if (time - last >= 50) {
        drawLevelMap(context, bundle, time - start, secrets);
        last = time;
      }
      if (visible && !document.hidden) frame = requestAnimationFrame(tick);
    };
    const resume = () => {
      cancelAnimationFrame(frame);
      if (!document.hidden) frame = requestAnimationFrame(tick);
    };
    tick(start);
    document.addEventListener("visibilitychange", resume);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("visibilitychange", resume);
    };
  }, [bundle, visible, secrets]);
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
      style={{ height }}
      tabIndex={0}
      role="region"
      aria-label={label}
      onPointerDown={pointDown}
      onPointerMove={pointMove}
      onPointerUp={pointUp}
      onPointerCancel={pointUp}
      onLostPointerCapture={pointUp}
      onKeyDown={(event) => {
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
    </div>
  );
}
