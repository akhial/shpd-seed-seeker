import { mapSpriteCache, makeFrameCanvas } from "./frame-cache";
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

/** Rasterize each sprite state once, then repair only changed animation bounds.
 * Replaying intersecting sprites in scene order preserves transparency and occlusion. */
export function createLevelMapRenderer(
  context: CanvasRenderingContext2D,
  bundle: MapBundle,
  revealSecrets: boolean,
  makeCanvas: () => HTMLCanvasElement = makeFrameCanvas,
) {
  const { map } = bundle;
  const cache = mapSpriteCache(bundle, makeCanvas);
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
      const cached = cache[index];
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
          const frame = entry.cached.frames[entry.frame];
          if (frame) {
            const [x, y, width, height] = frame.destination;
            context.drawImage(
              frame.image,
              ...frame.source,
              entry.x + x - entry.cached.x,
              entry.y + y - entry.cached.y,
              width,
              height,
            );
          }
        }
      }
      context.restore();
      initial = false;
    },
  };
}
