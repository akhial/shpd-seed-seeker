import type { MapBundle, MapSprite } from "./types";

export function spriteFrame(sprite: MapSprite, elapsed: number) {
  return sprite.frames[
    Math.floor(Math.max(0, elapsed) / Math.max(1, sprite.frameDurationMs)) % sprite.frames.length
  ];
}
export function drawLevelMap(
  context: CanvasRenderingContext2D,
  { map, textures }: MapBundle,
  elapsed: number,
  revealSecrets: boolean,
) {
  const tileSize = map.scene.tileSize;
  context.clearRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.fillStyle = "#000";
  context.fillRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.imageSmoothingEnabled = false;
  for (const layer of revealSecrets ? map.scene.layers : map.scene.concealedLayers) {
    for (let cell = 0; cell < layer.cells.length; cell++) {
      const index = layer.cells[cell];
      if (index === null) continue;
      const ox = (cell % map.width) * tileSize;
      const oy = Math.floor(cell / map.width) * tileSize;
      for (const draw of spriteFrame(map.scene.sprites[index], elapsed)) {
        const [x, y, w, h] = draw.destination;
        if (draw.kind === "blit") {
          context.drawImage(textures.get(draw.asset)!, ...draw.source, ox + x, oy + y, w, h);
        } else {
          const [r, g, b, a] = draw.rgba;
          context.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
          context.fillRect(ox + x, oy + y, w, h);
        }
      }
    }
  }
}

interface CachedSprite {
  frames: HTMLCanvasElement[];
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Rasterize each sprite state once, then repair only changed animation bounds.
 * Replaying intersecting sprites in scene order preserves transparency and occlusion. */
export function createLevelMapRenderer(
  context: CanvasRenderingContext2D,
  { map, textures }: MapBundle,
  revealSecrets: boolean,
  makeCanvas: () => HTMLCanvasElement = () => document.createElement("canvas"),
) {
  const cache = new Map<number, CachedSprite>();
  const coveredCells = (x: number, y: number, width: number, height: number) => {
    const cells: number[] = [];
    const size = map.scene.tileSize;
    for (
      let row = Math.max(0, Math.floor(y / size));
      row < Math.min(map.height, Math.ceil((y + height) / size));
      row++
    ) {
      for (
        let column = Math.max(0, Math.floor(x / size));
        column < Math.min(map.width, Math.ceil((x + width) / size));
        column++
      ) {
        cells.push(row * map.width + column);
      }
    }
    return cells;
  };
  const entries = (revealSecrets ? map.scene.layers : map.scene.concealedLayers).flatMap((layer) =>
    layer.cells.flatMap((index, cell) => {
      if (index === null) return [];
      const sprite = map.scene.sprites[index];
      let cached = cache.get(index);
      if (!cached) {
        const draws = sprite.frames.flat();
        const x = Math.floor(Math.min(0, ...draws.map((draw) => draw.destination[0])));
        const y = Math.floor(Math.min(0, ...draws.map((draw) => draw.destination[1])));
        const width =
          Math.ceil(
            Math.max(0, ...draws.map((draw) => draw.destination[0] + draw.destination[2])),
          ) - x;
        const height =
          Math.ceil(
            Math.max(0, ...draws.map((draw) => draw.destination[1] + draw.destination[3])),
          ) - y;
        const frames = sprite.frames.map((draws) => {
          const canvas = makeCanvas();
          canvas.width = Math.max(1, width);
          canvas.height = Math.max(1, height);
          const target = canvas.getContext("2d")!;
          target.imageSmoothingEnabled = false;
          for (const draw of draws) {
            const [dx, dy, w, h] = draw.destination;
            if (draw.kind === "blit") {
              target.drawImage(textures.get(draw.asset)!, ...draw.source, dx - x, dy - y, w, h);
            } else {
              const [r, g, b, a] = draw.rgba;
              target.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
              target.fillRect(dx - x, dy - y, w, h);
            }
          }
          return canvas;
        });
        cached = { frames, x, y, width, height };
        cache.set(index, cached);
      }
      const x = (cell % map.width) * map.scene.tileSize + cached.x;
      const y = Math.floor(cell / map.width) * map.scene.tileSize + cached.y;
      return [
        { sprite, cached, x, y, cells: coveredCells(x, y, cached.width, cached.height), frame: -1 },
      ];
    }),
  );
  let initial = true;
  return {
    animated: entries.some(({ sprite }) => sprite.frames.length > 1),
    draw(elapsed: number) {
      const changed = entries.filter((entry) => {
        const frame =
          Math.floor(Math.max(0, elapsed) / Math.max(1, entry.sprite.frameDurationMs)) %
          entry.sprite.frames.length;
        if (entry.frame === frame) return false;
        entry.frame = frame;
        return true;
      });
      if (!initial && changed.length === 0) return;
      const dirtyCells = new Set(changed.flatMap((entry) => entry.cells));
      context.save();
      if (!initial) {
        context.beginPath();
        for (const entry of changed) {
          context.rect(entry.x, entry.y, entry.cached.width, entry.cached.height);
        }
        context.clip();
      }
      context.fillStyle = "#000";
      context.fillRect(0, 0, map.width * map.scene.tileSize, map.height * map.scene.tileSize);
      context.imageSmoothingEnabled = false;
      for (const entry of entries) {
        if (initial || entry.cells.some((cell) => dirtyCells.has(cell))) {
          context.drawImage(entry.cached.frames[entry.frame], entry.x, entry.y);
        }
      }
      context.restore();
      initial = false;
    },
  };
}
