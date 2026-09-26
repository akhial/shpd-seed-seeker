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
  const x = Math.floor(
    ((point.x - transform.x) / scale + geometry.mapWidth / 2) / map.scene.tileSize,
  );
  const y = Math.floor(
    ((point.y - transform.y) / scale + geometry.mapHeight / 2) / map.scene.tileSize,
  );
  if (x < 0 || y < 0 || x >= map.width || y >= map.height) return;
  return map.itemTooltips?.find(
    (tip) => tip.cell === y * map.width + x && (!tip.hidden || secrets),
  );
}
