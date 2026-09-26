import type { LevelMapDocument, MapItemTooltip } from "./types";
import { mapFitScale, type MapGeometry, type MapPoint, type MapTransform } from "./map-gestures";

/** Pointer coordinates relative to the viewport center, matching map gestures. */
export function itemAtPoint(
  map: Pick<LevelMapDocument, "width" | "height" | "itemTooltips"> & {
    scene: Pick<LevelMapDocument["scene"], "tileSize">;
  },
  point: MapPoint,
  geometry: MapGeometry,
  transform: MapTransform,
  secrets: boolean,
): MapItemTooltip | undefined {
  const scale = mapFitScale(geometry) * transform.zoom;
  const x = (point.x - transform.x) / scale + geometry.mapWidth / 2;
  const y = (point.y - transform.y) / scale + geometry.mapHeight / 2;
  if (x < 0 || y < 0 || x >= geometry.mapWidth || y >= geometry.mapHeight) return;
  return map.itemTooltips?.find((tip) => {
    const [left, top, width, height] = itemBounds(tip, map.width, map.scene.tileSize);
    return (
      (!tip.hidden || secrets) && x >= left && y >= top && x < left + width && y < top + height
    );
  });
}

/** The same raised sprite rectangle drives hit testing and the visible selector. */
export function itemBounds(tip: MapItemTooltip, columns: number, tile: number) {
  const [x, y, width, height] = tip.bounds ?? [0, 0, tile, tile];
  return [
    (tip.cell % columns) * tile + x,
    Math.floor(tip.cell / columns) * tile + y,
    width,
    height,
  ];
}
