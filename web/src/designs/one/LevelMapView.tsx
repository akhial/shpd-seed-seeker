import { useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import { mapRequestJson, requestLevelMap } from "../../lib/level-map/client";
import { drawLevelMap } from "../../lib/level-map/render";
import type { LevelMapRequest, MapBundle } from "../../lib/level-map/types";
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
      {branches.length > 0 && (
        <div className="d1-map-branches" role="group" aria-label="Level area">
          <button type="button" aria-pressed={branch === 0} onClick={() => setBranch(0)}>
            Floor {props.depth}
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
}: {
  bundle: MapBundle;
  height: number;
  label: string;
}) {
  const { map } = bundle;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 300, height });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [secrets, setSecrets] = useState(false);
  const [playing, setPlaying] = useState(
    () =>
      typeof window !== "undefined" &&
      !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
  );
  const [visible, setVisible] = useState(true);
  const drag = useRef<{ id: number; x: number; y: number; startX: number; startY: number } | null>(
    null,
  );
  const widthPx = map.width * map.scene.tileSize,
    heightPx = map.height * map.scene.tileSize;
  const fit = Math.min((size.width - 24) / widthPx, (size.height - 24) / heightPx);
  const scale = Math.max(0.01, fit) * zoom;
  const boundX = Math.max(0, (widthPx * scale - size.width) / 2 + 36);
  const boundY = Math.max(0, (heightPx * scale - size.height) / 2 + 36);
  // Keep the map reachable after zooming out or resizing an already panned view.
  useEffect(() => {
    setPan((value) => {
      const x = Math.max(-boundX, Math.min(boundX, value.x));
      const y = Math.max(-boundY, Math.min(boundY, value.y));
      return x === value.x && y === value.y ? value : { x, y };
    });
  }, [boundX, boundY]);
  const secretCount = map.secretRooms.length + map.secretDoors.length + map.secretTraps.length;
  const reset = () => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  };
  const changeZoom = (value: number) => {
    const next = Math.max(1, Math.min(6, value));
    setZoom(next);
    if (next === 1) setPan({ x: 0, y: 0 });
  };
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
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => {
      if (preference.matches) setPlaying(false);
    };
    preference.addEventListener("change", update);
    return () => preference.removeEventListener("change", update);
  }, []);
  useEffect(() => {
    const context = canvasRef.current?.getContext("2d");
    if (!context) return;
    let frame = 0,
      last = -Infinity;
    const start = performance.now();
    const tick = (time: number) => {
      if (time - last >= 50) {
        drawLevelMap(context, bundle, playing ? time - start : 0, secrets);
        last = time;
      }
      if (playing && visible && !document.hidden) frame = requestAnimationFrame(tick);
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
  }, [bundle, playing, visible, secrets]);
  const pointDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || zoom === 1) return;
    event.stopPropagation();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      startX: pan.x,
      startY: pan.y,
    };
  };
  const clampPan = (x: number, y: number) => {
    return { x: Math.max(-boundX, Math.min(boundX, x)), y: Math.max(-boundY, Math.min(boundY, y)) };
  };
  const pointMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const current = drag.current;
    if (!current || current.id !== event.pointerId) return;
    setPan(
      clampPan(
        current.startX + event.clientX - current.x,
        current.startY + event.clientY - current.y,
      ),
    );
  };
  const endDrag = () => {
    drag.current = null;
  };
  const canvasStyle: CSSProperties = {
    width: widthPx * scale,
    height: heightPx * scale,
    transform: `translate(calc(-50% + ${pan.x}px), calc(-50% + ${pan.y}px))`,
  };
  return (
    <>
      <div className="d1-map-toolbar">
        <div className="d1-map-zoom" role="group" aria-label="Map zoom">
          <button
            type="button"
            aria-label="Zoom out"
            disabled={zoom === 1}
            onClick={() => changeZoom(zoom / 1.5)}
          >
            −
          </button>
          <button type="button" onClick={reset} title="Fit the entire map">
            Fit
          </button>
          <button
            type="button"
            aria-label="Zoom in"
            disabled={zoom === 6}
            onClick={() => changeZoom(zoom * 1.5)}
          >
            +
          </button>
        </div>
        <button
          type="button"
          aria-pressed={secrets}
          disabled={secretCount === 0}
          onClick={() => setSecrets((value) => !value)}
          title="Highlight secret rooms, doors and traps"
        >
          Secrets{secretCount > 0 ? ` · ${secretCount}` : ""}
        </button>
        <button
          type="button"
          className="d1-map-motion"
          aria-pressed={playing}
          onClick={() => setPlaying((value) => !value)}
          aria-label={playing ? "Pause map animation" : "Play map animation"}
        >
          {playing ? "Pause" : "Animate"}
        </button>
      </div>
      <div
        ref={viewportRef}
        className={`d1-map-viewport ${zoom > 1 ? "d1-map-zoomed" : ""}`}
        style={{ height, touchAction: zoom > 1 ? "none" : "pan-y" }}
        tabIndex={0}
        role="region"
        aria-label={`${label}. Use zoom buttons, drag to pan, or arrow keys when zoomed.`}
        onPointerDown={pointDown}
        onPointerMove={pointMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onLostPointerCapture={endDrag}
        onKeyDown={(event) => {
          if (
            ["+", "=", "-", "0", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(
              event.key,
            )
          ) {
            event.stopPropagation();
            event.preventDefault();
            if (event.key === "0") reset();
            else if (event.key === "+" || event.key === "=") changeZoom(zoom * 1.5);
            else if (event.key === "-") changeZoom(zoom / 1.5);
            else if (zoom > 1)
              setPan((value) =>
                clampPan(
                  value.x + (event.key === "ArrowLeft" ? 24 : event.key === "ArrowRight" ? -24 : 0),
                  value.y + (event.key === "ArrowUp" ? 24 : event.key === "ArrowDown" ? -24 : 0),
                ),
              );
          }
        }}
      >
        <canvas
          ref={canvasRef}
          width={widthPx}
          height={heightPx}
          style={canvasStyle}
          role="img"
          aria-label={`${label}: ${map.width} by ${map.height} tiles, ${map.secretRooms.length} secret rooms, ${map.traps.length} traps.`}
        />
        {zoom > 1 && (
          <span className="d1-map-pan-hint" aria-hidden="true">
            Drag to explore
          </span>
        )}
      </div>
      <div className="d1-map-legend">
        <span>
          <i className="d1-map-dot-entry" />
          {map.branch ? "Return" : "Entrance"}
        </span>
        {map.exit !== null && (
          <span>
            <i className="d1-map-dot-exit" />
            Exit
          </span>
        )}
        {map.branches.length > 0 && (
          <span>
            <i className="d1-map-dot-quest" />
            Quest
          </span>
        )}
        <span className="d1-map-size">
          {map.width} × {map.height}
        </span>
      </div>
      <p className="d1-map-caption">
        {map.kind === "blacksmith_crystal"
          ? "Crystal mine · "
          : map.kind === "blacksmith_gnoll"
            ? "Gnoll mine · "
            : ""}
        Full layout · Secrets revealed · Terrain and traps
      </p>
    </>
  );
}
