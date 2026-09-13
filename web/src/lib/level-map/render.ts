import { drawTexture, glowTexture } from "./textures";
import { mapSpriteCache, makeFrameCanvas } from "./frame-cache";
import type { MapBundle, MapDraw, MapSprite } from "./types";

export function spriteFrame(sprite: MapSprite, elapsed: number) {
  return sprite.frames[
    Math.floor(Math.max(0, elapsed) / Math.max(1, sprite.frameDurationMs)) % sprite.frames.length
  ];
}
export function drawLevelMap(
  context: CanvasRenderingContext2D,
  bundle: MapBundle,
  elapsed: number,
  revealSecrets: boolean,
) {
  const { map } = bundle;
  const tileSize = map.scene.tileSize;
  context.globalCompositeOperation = "source-over";
  context.globalAlpha = 1;
  context.clearRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.fillStyle = "#000";
  context.fillRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.imageSmoothingEnabled = false;
  for (const layer of revealSecrets ? map.scene.layers : map.scene.concealedLayers) {
    context.globalCompositeOperation = layer.blend === "add" ? "lighter" : "source-over";
    for (let cell = 0; cell < layer.cells.length; cell++) {
      const index = layer.cells[cell];
      if (index === null) continue;
      const ox = (cell % map.width) * tileSize;
      const oy = Math.floor(cell / map.width) * tileSize;
      for (const draw of spriteFrame(map.scene.sprites[index], elapsed)) {
        drawCommand(context, bundle, draw, ox, oy, elapsed);
      }
    }
  }
  context.globalAlpha = 1;
  context.globalCompositeOperation = "source-over";
}

export function glowAmount(periodMs: number, elapsed: number): number {
  const phase = (Math.max(0, elapsed) / Math.max(1, periodMs)) % 2;
  return (phase <= 1 ? phase : 2 - phase) * 0.6;
}

export function drawCommand(
  context: CanvasRenderingContext2D,
  bundle: MapBundle,
  draw: MapDraw,
  ox: number,
  oy: number,
  elapsed = 0,
) {
  const [x, y, w, h] = draw.destination;
  context.globalAlpha = draw.kind === "blit" ? (draw.opacity ?? 255) / 255 : 1;
  if (draw.kind === "blit") {
    if (draw.glow) {
      context.drawImage(
        glowTexture(bundle, draw, glowAmount(draw.glow.periodMs, elapsed)),
        0,
        0,
        draw.source[2],
        draw.source[3],
        ox + x,
        oy + y,
        w,
        h,
      );
    } else {
      context.drawImage(drawTexture(bundle, draw), ...draw.source, ox + x, oy + y, w, h);
    }
  } else {
    const [r, g, b, a] = draw.rgba;
    context.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
    context.fillRect(ox + x, oy + y, w, h);
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
      const frames =
        layer.blend === "add" ? (cached.additiveFrames ?? cached.frames) : cached.frames;
      const x = (cell % map.width) * map.scene.tileSize + cached.x;
      const y = Math.floor(cell / map.width) * map.scene.tileSize + cached.y;
      return [
        {
          sprite,
          cached,
          frames,
          blend: layer.blend,
          x,
          y,
          cells: coveredCells(x, y, cached.width, cached.height),
          frame: -1,
          glowing: sprite.frames.some((frame) =>
            frame.some((draw) => draw.kind === "blit" && draw.glow),
          ),
        },
      ];
    }),
  );
  let initial = true;
  let previousElapsed = -1;
  return {
    animated: entries.some(({ sprite, glowing }) => sprite.frames.length > 1 || glowing),
    draw(elapsed: number, advanceSprites = true) {
      const changed = entries.filter((entry) => {
        const frame =
          !advanceSprites && entry.frame >= 0
            ? entry.frame
            : Math.floor(Math.max(0, elapsed) / Math.max(1, entry.sprite.frameDurationMs)) %
              entry.sprite.frames.length;
        if (entry.frame === frame && !(entry.glowing && elapsed !== previousElapsed)) return false;
        entry.frame = frame;
        return true;
      });
      previousElapsed = elapsed;
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
      context.globalAlpha = 1;
      context.globalCompositeOperation = "source-over";
      context.fillStyle = "#000";
      context.fillRect(0, 0, map.width * map.scene.tileSize, map.height * map.scene.tileSize);
      context.imageSmoothingEnabled = false;
      for (const entry of entries) {
        if (initial || entry.cells.some((cell) => dirtyCells.has(cell))) {
          const frame = entry.frames[entry.frame];
          if (frame) {
            context.globalCompositeOperation = entry.blend === "add" ? "lighter" : "source-over";
            if (frame.commands) {
              for (const draw of frame.commands)
                drawCommand(
                  context,
                  bundle,
                  draw,
                  entry.x - entry.cached.x,
                  entry.y - entry.cached.y,
                  elapsed,
                );
              continue;
            }
            context.globalAlpha = frame.opacity / 255;
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
