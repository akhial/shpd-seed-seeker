import type { MapBundle, MapDraw, Rectangle } from "./types";

interface CachedFrame {
  image: CanvasImageSource;
  source: Rectangle;
  destination: Rectangle;
}
export interface CachedSprite {
  frames: (CachedFrame | null)[];
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
function buildSpriteCache({ map, textures }: MapBundle, makeCanvas: () => HTMLCanvasElement) {
  type Page = {
    canvas: HTMLCanvasElement;
    width: number;
    height: number;
    x: number;
    y: number;
    rowHeight: number;
    draws: { commands: MapDraw[]; source: Rectangle; destination: Rectangle }[];
  };
  const pages: Page[] = [];
  const frames = new Map<string, CachedFrame | null>();
  const sprites = map.scene.sprites.map((sprite): CachedSprite => {
    const [x, y, width, height] = bounds(sprite.frames.flat());
    return {
      x,
      y,
      width,
      height,
      frames: sprite.frames.map((commands) => {
        const key = JSON.stringify(commands);
        if (frames.has(key)) return frames.get(key)!;
        let frame: CachedFrame | null;
        if (!commands.length) {
          frame = null;
        } else if (commands.length === 1 && commands[0].kind === "blit") {
          const draw = commands[0];
          frame = {
            image: textures.get(draw.asset)!,
            source: draw.source,
            destination: draw.destination,
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
          frame = { image: page.canvas, source, destination };
          page.draws.push({ commands, source, destination });
          page.x += w;
          page.rowHeight = Math.max(page.rowHeight, h);
          page.height = Math.max(page.height, page.y + h);
        }
        frames.set(key, frame);
        return frame;
      }),
    };
  });
  for (const page of pages) {
    page.canvas.width = page.width;
    page.canvas.height = page.height;
    const target = page.canvas.getContext("2d")!;
    target.imageSmoothingEnabled = false;
    for (const { commands, source, destination } of page.draws) {
      for (const draw of commands) {
        const [dx, dy, w, h] = draw.destination;
        const x = source[0] + dx - destination[0];
        const y = source[1] + dy - destination[1];
        if (draw.kind === "blit") {
          target.drawImage(textures.get(draw.asset)!, ...draw.source, x, y, w, h);
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
