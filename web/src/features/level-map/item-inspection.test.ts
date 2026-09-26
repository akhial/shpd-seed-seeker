import { expect, it } from "vite-plus/test";
import { itemAtPoint, itemBounds } from "./item-inspection";
import type { MapItemTooltip } from "./types";

it("selects the raised chest sprite, including its overhang above the ground tile", () => {
  const tip: MapItemTooltip = {
    cell: 5,
    label: "Locked chest",
    hidden: false,
    items: [],
    bounds: [0, -3, 16, 14],
  };
  const map = { width: 4, height: 3, scene: { tileSize: 16 }, itemTooltips: [tip] };
  const geometry = { width: 88, height: 72, mapWidth: 64, mapHeight: 48 };
  expect(itemBounds(tip, 4, 16)).toEqual([16, 13, 16, 14]);
  for (const zoom of [1, 3]) {
    const transform = { x: 13, y: -7, zoom };
    const point = (x: number, y: number) => ({ x: (x - 32) * zoom + 13, y: (y - 24) * zoom - 7 });
    expect(itemAtPoint(map, point(24, 14), geometry, transform, false)).toBe(tip);
    expect(itemAtPoint(map, point(24, 28), geometry, transform, false)).toBeUndefined();
  }
});
it("inverts zoom and pan, rejects margins, and hides secret loot", () => {
  const tip = { cell: 5, label: "", hidden: false, items: [] };
  const secret = { ...tip, cell: 6, hidden: true };
  const map = {
    width: 4,
    height: 3,
    scene: { tileSize: 16 },
    itemTooltips: [tip, secret],
  };
  const geometry = { width: 88, height: 72, mapWidth: 64, mapHeight: 48 };
  for (const transform of [
    { x: 0, y: 0, zoom: 1 },
    { x: 13, y: -7, zoom: 3 },
  ]) {
    const point = { x: -8 * transform.zoom + transform.x, y: transform.y };
    expect(itemAtPoint(map, point, geometry, transform, false)).toBe(tip);
    const hiddenPoint = { ...point, x: 8 * transform.zoom + transform.x };
    expect(itemAtPoint(map, hiddenPoint, geometry, transform, false)).toBeUndefined();
    expect(itemAtPoint(map, hiddenPoint, geometry, transform, true)).toBe(secret);
    expect(
      itemAtPoint(map, { x: -33 * transform.zoom + transform.x, y: 0 }, geometry, transform, true),
    ).toBeUndefined();
  }
  expect(
    itemAtPoint(
      { ...map, itemTooltips: undefined },
      { x: 0, y: 0 },
      geometry,
      { x: 0, y: 0, zoom: 1 },
      true,
    ),
  ).toBeUndefined();
});
