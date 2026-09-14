export interface MapPoint {
  x: number;
  y: number;
}

export interface MapTransform extends MapPoint {
  zoom: number;
}

export interface MapGeometry {
  width: number;
  height: number;
  mapWidth: number;
  mapHeight: number;
}

export const FIT_MAP: MapTransform = { zoom: 1, x: 0, y: 0 };

const clamp = (value: number, minimum: number, maximum: number) =>
  Math.max(minimum, Math.min(maximum, value));

export function mapFitScale(geometry: MapGeometry): number {
  return Math.max(
    0.01,
    Math.min(
      (geometry.width - 24) / geometry.mapWidth,
      (geometry.height - 24) / geometry.mapHeight,
    ),
  );
}

/** Keep the layout reachable, including when a zoom or resize reduces its bounds. */
export function constrainMapTransform(
  transform: MapTransform,
  geometry: MapGeometry,
): MapTransform {
  const zoom = clamp(transform.zoom, 1, 6);
  if (zoom === 1) return FIT_MAP;
  const scale = mapFitScale(geometry) * zoom;
  const boundX = Math.max(0, (geometry.mapWidth * scale - geometry.width) / 2 + 36);
  const boundY = Math.max(0, (geometry.mapHeight * scale - geometry.height) / 2 + 36);
  return { zoom, x: clamp(transform.x, -boundX, boundX), y: clamp(transform.y, -boundY, boundY) };
}

/** Anchor coordinates are relative to the viewport center, like the map translation. */
export function zoomMapAt(
  transform: MapTransform,
  zoom: number,
  anchor: MapPoint,
  geometry: MapGeometry,
): MapTransform {
  const nextZoom = clamp(zoom, 1, 6);
  const ratio = nextZoom / transform.zoom;
  return constrainMapTransform(
    {
      zoom: nextZoom,
      x: anchor.x - (anchor.x - transform.x) * ratio,
      y: anchor.y - (anchor.y - transform.y) * ratio,
    },
    geometry,
  );
}

/** Normalize line/page wheels as well as pixel-based trackpads and pinch-wheel events. */
export function wheelZoomFactor(delta: number, deltaMode: number, controlKey: boolean): number {
  const pixels = delta * (deltaMode === 1 ? 16 : deltaMode === 2 ? 350 : 1);
  return Math.exp(-clamp(pixels, -200, 200) * (controlKey ? 0.01 : 0.002));
}

const midpoint = (left: MapPoint, right: MapPoint): MapPoint => ({
  x: (left.x + right.x) / 2,
  y: (left.y + right.y) / 2,
});
const distance = (left: MapPoint, right: MapPoint) =>
  Math.hypot(left.x - right.x, left.y - right.y);

/** Pointer changes rebase the gesture so pan → pinch → pan never jumps. */
export class MapGesture {
  private readonly pointers = new Map<number, MapPoint>();
  private start?: { points: [number, MapPoint][]; transform: MapTransform };

  rebase(transform: MapTransform): void {
    this.start = { points: [...this.pointers].slice(0, 2), transform };
  }

  down(id: number, point: MapPoint, transform: MapTransform): void {
    this.pointers.set(id, point);
    this.rebase(transform);
  }

  move(id: number, point: MapPoint, geometry: MapGeometry): MapTransform | undefined {
    if (!this.pointers.has(id) || !this.start) return undefined;
    this.pointers.set(id, point);
    const { points, transform } = this.start;
    const [first, second] = points;
    if (!first || !points.some(([pointerId]) => pointerId === id)) return undefined;
    const currentFirst = this.pointers.get(first[0])!;
    if (!second) {
      return constrainMapTransform(
        {
          zoom: transform.zoom,
          x: transform.x + currentFirst.x - first[1].x,
          y: transform.y + currentFirst.y - first[1].y,
        },
        geometry,
      );
    }
    const currentSecond = this.pointers.get(second[0])!;
    const initialDistance = distance(first[1], second[1]);
    const nextZoom = clamp(
      transform.zoom * (distance(currentFirst, currentSecond) / Math.max(1, initialDistance)),
      1,
      6,
    );
    const ratio = nextZoom / transform.zoom;
    const from = midpoint(first[1], second[1]);
    const to = midpoint(currentFirst, currentSecond);
    return constrainMapTransform(
      {
        zoom: nextZoom,
        x: to.x - (from.x - transform.x) * ratio,
        y: to.y - (from.y - transform.y) * ratio,
      },
      geometry,
    );
  }

  up(id: number, transform: MapTransform): void {
    if (this.pointers.delete(id)) this.rebase(transform);
  }
}
