import { drawTexture } from "./textures";
import type { MapBundle, MapDraw, Rectangle } from "./types";

interface CachedFrame {
  commands?: MapDraw[];
  opacity: number;
  image: CanvasImageSource;
  source: Rectangle;
  destination: Rectangle;
}
export interface CachedSprite {
  frames: (CachedFrame | null)[];
  additiveFrames?: (CachedFrame | null)[];
  x: number;
  y: number;
  width: number;
  height: number;
}

export const makeFrameCanvas = () => document.createElement("canvas");
const sharedCaches = new WeakMap<MapBundle, CachedSprite[]>();

/** Share immutable frame images across secret toggles and reopenings of a cached map.
 * Weak ownership lets evicted map bundles and their atlases be collected together. */
export function mapSpriteCache(bundle: MapBundle, makeCanvas = makeFrameCanvas): CachedSprite[] {
  if (makeCanvas !== makeFrameCanvas) return buildSpriteCache(bundle, makeCanvas);
  let cached = sharedCaches.get(bundle);
  if (!cached) {
    cached = buildSpriteCache(bundle, makeCanvas);
    sharedCaches.set(bundle, cached);
  }
  return cached;
}

function bounds(draws: MapDraw[]): Rectangle {
  if (!draws.length) return [0, 0, 0, 0];
  const x = Math.floor(Math.min(...draws.map((draw) => draw.destination[0])));
  const y = Math.floor(Math.min(...draws.map((draw) => draw.destination[1])));
  return [
    x,
    y,
    Math.ceil(Math.max(...draws.map((draw) => draw.destination[0] + draw.destination[2]))) - x,
    Math.ceil(Math.max(...draws.map((draw) => draw.destination[1] + draw.destination[3]))) - y,
  ];
}

/** Single blits use the original texture. Only composite frames need new pixels;
 * identical commands (including phase-shifted water frames) share one atlas slot. */
function buildSpriteCache(bundle: MapBundle, makeCanvas: () => HTMLCanvasElement) {
  const { map } = bundle;
  type Page = {
    canvas: HTMLCanvasElement;
    width: number;
    height: number;
    x: number;
    y: number;
    rowHeight: number;
    draws: { additive: boolean; commands: MapDraw[]; source: Rectangle; destination: Rectangle }[];
  };
  const pages: Page[] = [];
  const frames = new Map<string, CachedFrame | null>();
  const additiveSprites = new Set(
    [...map.scene.layers, ...map.scene.concealedLayers]
      .filter((layer) => layer.blend === "add")
      .flatMap((layer) => layer.cells),
  );
  const sprites = map.scene.sprites.map((sprite, index): CachedSprite => {
    const rasterFrames = (additive: boolean) =>
      sprite.frames.map((commands) => {
        const key = JSON.stringify([additive, commands]);
        if (frames.has(key)) return frames.get(key)!;
        let frame: CachedFrame | null;
        if (!commands.length) {
          frame = null;
        } else if (commands.length === 1 && commands[0].kind === "blit") {
          const draw = commands[0];
          frame = {
            opacity: draw.opacity ?? 255,
            image: drawTexture(bundle, draw, makeCanvas),
            source: draw.source,
            destination: draw.destination,
          };
        } else if (
          commands.some(
            (draw) =>
              draw.kind === "blit" &&
              (draw.source[2] !== draw.destination[2] || draw.source[3] !== draw.destination[3]),
          )
        ) {
          // Browser nearest-neighbour ties depend on the destination origin.
          // Replay scaled composites at their final position so shadows do not
          // change when packed into an atlas at a different origin.
          frame = {
            commands,
            opacity: 255,
            image: texturesPlaceholder(bundle),
            source: [0, 0, 0, 0],
            destination: bounds(commands),
          };
        } else {
          const destination = bounds(commands);
          const [, , w, h] = destination;
          let page = pages.at(-1);
          if (page && page.x + w > page.width) {
            page.x = 0;
            page.y += page.rowHeight;
            page.rowHeight = 0;
          }
          if (!page || w > page.width || page.y + h > 1024) {
            page = {
              canvas: makeCanvas(),
              width: Math.max(256, w),
              height: 0,
              x: 0,
              y: 0,
              rowHeight: 0,
              draws: [],
            };
            pages.push(page);
          }
          const source: Rectangle = [page.x, page.y, w, h];
          frame = { opacity: 255, image: page.canvas, source, destination };
          page.draws.push({ additive, commands, source, destination });
          page.x += w;
          page.rowHeight = Math.max(page.rowHeight, h);
          page.height = Math.max(page.height, page.y + h);
        }
        frames.set(key, frame);
        return frame;
      });
    const [x, y, width, height] = bounds(sprite.frames.flat());
    return {
      x,
      y,
      width,
      height,
      frames: rasterFrames(false),
      additiveFrames: additiveSprites.has(index) ? rasterFrames(true) : undefined,
    };
  });
  for (const page of pages) {
    page.canvas.width = page.width;
    page.canvas.height = page.height;
    const target = page.canvas.getContext("2d")!;
    target.imageSmoothingEnabled = false;
    for (const { additive, commands, source, destination } of page.draws) {
      target.globalCompositeOperation = additive ? "lighter" : "source-over";
      for (const draw of commands) {
        const [dx, dy, w, h] = draw.destination;
        const x = source[0] + dx - destination[0];
        const y = source[1] + dy - destination[1];
        target.globalAlpha = draw.kind === "blit" ? (draw.opacity ?? 255) / 255 : 1;
        if (draw.kind === "blit") {
          target.drawImage(drawTexture(bundle, draw, makeCanvas), ...draw.source, x, y, w, h);
        } else {
          const [r, g, b, a] = draw.rgba;
          target.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
          target.fillRect(x, y, w, h);
        }
      }
    }
  }
  return sprites;
}

function texturesPlaceholder(bundle: MapBundle): CanvasImageSource {
  return bundle.textures.values().next().value!;
}
