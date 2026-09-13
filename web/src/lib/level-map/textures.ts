import type { MapBundle, MapDraw } from "./types";

const tinted = new WeakMap<MapBundle, Map<string, CanvasImageSource>>();
/** The game multiplies RGB independently of source alpha (including black shadows). */
export function drawTexture(
  bundle: MapBundle,
  draw: Extract<MapDraw, { kind: "blit" }>,
  makeCanvas = () => document.createElement("canvas"),
): CanvasImageSource {
  const original = bundle.textures.get(draw.asset)!;
  if (!draw.tint) return original;
  let cache = tinted.get(bundle);
  if (!cache) {
    cache = new Map();
    tinted.set(bundle, cache);
  }
  const key = `${draw.asset}:${draw.tint.join(",")}`;
  let image = cache.get(key);
  if (!image) {
    const canvas = makeCanvas();
    const asset = bundle.map.assets.find((asset) => asset.id === draw.asset)!;
    canvas.width = asset.width;
    canvas.height = asset.height;
    const context = canvas.getContext("2d")!;
    context.fillStyle = `rgb(${draw.tint.join(",")})`;
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.globalCompositeOperation = "multiply";
    context.drawImage(original, 0, 0);
    context.globalCompositeOperation = "destination-in";
    context.drawImage(original, 0, 0);
    image = canvas;
    cache.set(key, image);
  }
  return image;
}
