import { makeMapCanvas, mapContext, type MapCanvas } from "./canvas";
import type { MapBundle, MapDraw } from "../types";

const tinted = new WeakMap<MapBundle, Map<string, CanvasImageSource>>();
type GlowingTexture = { image: MapCanvas; amount: number };
const glowing = new WeakMap<MapBundle, Map<string, GlowingTexture>>();

/** Mix RGB inside a small sprite buffer, preserving even translucent source pixels. */
export function glowTexture(
  bundle: MapBundle,
  draw: Extract<MapDraw, { kind: "blit" }>,
  amount: number,
): CanvasImageSource {
  let cache = glowing.get(bundle);
  if (!cache) {
    cache = new Map();
    glowing.set(bundle, cache);
  }
  const key = JSON.stringify([draw.asset, draw.source, draw.tint, draw.glow!.color]);
  let entry = cache.get(key);
  const [, , width, height] = draw.source;
  if (!entry) {
    const canvas = makeMapCanvas();
    canvas.width = width;
    canvas.height = height;
    entry = { image: canvas, amount: -1 };
    cache.set(key, entry);
  }
  if (entry.amount !== amount) {
    const context = mapContext(entry.image);
    context.globalAlpha = 1;
    context.globalCompositeOperation = "copy";
    context.drawImage(drawTexture(bundle, draw), ...draw.source, 0, 0, width, height);
    context.globalCompositeOperation = "source-atop";
    context.globalAlpha = amount;
    context.fillStyle = `rgb(${draw.glow!.color.join(",")})`;
    context.fillRect(0, 0, width, height);
    entry.amount = amount;
  }
  return entry.image;
}
/** The game multiplies RGB independently of source alpha (including black shadows). */
export function drawTexture(
  bundle: MapBundle,
  draw: Extract<MapDraw, { kind: "blit" }>,
  makeCanvas = makeMapCanvas,
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
    const context = mapContext(canvas);
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
