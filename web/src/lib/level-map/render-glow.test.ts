import { expect, it, vi } from "vite-plus/test";
import { createLevelMapRenderer } from "./render";
import type { MapBundle } from "./types";

it("updates heap glows between scenery ticks and repaints their occluding walls", () => {
  const item = {} as ImageBitmap,
    wall = {} as ImageBitmap,
    distant = {} as ImageBitmap;
  const amounts: number[] = [];
  const mask = {
    getContext: () => ({
      globalAlpha: 1,
      fillRect() {
        amounts.push(this.globalAlpha);
      },
      drawImage() {},
    }),
  };
  vi.stubGlobal("document", { createElement: () => mask });
  const draw = (asset: string, glow?: object) => ({
    kind: "blit",
    asset,
    source: [0, 0, 16, 16],
    destination: [0, 0, 16, 16],
    glow,
  });
  const bundle = {
    textures: new Map([
      ["item", item],
      ["wall", wall],
      ["distant", distant],
    ]),
    map: {
      width: 2,
      height: 1,
      assets: [{ id: "item", width: 16, height: 16 }],
      scene: {
        tileSize: 16,
        sprites: [
          { frameDurationMs: 1, frames: [[draw("item", { color: [0, 0, 255], periodMs: 1000 })]] },
          { frameDurationMs: 1, frames: [[draw("wall")]] },
          { frameDurationMs: 1, frames: [[draw("distant")]] },
        ],
        layers: [
          { name: "heaps", cells: [0, 2] },
          { name: "walls", cells: [1, null] },
        ],
        concealedLayers: [],
      },
    },
  } as unknown as MapBundle;
  const calls: { image: CanvasImageSource; alpha: number }[] = [];
  const context = {
    globalAlpha: 1,
    save() {},
    restore() {},
    beginPath() {},
    rect() {},
    clip() {},
    fillRect() {},
    drawImage(image: CanvasImageSource) {
      calls.push({ image, alpha: this.globalAlpha });
    },
  };
  try {
    const renderer = createLevelMapRenderer(
      context as unknown as CanvasRenderingContext2D,
      bundle,
      true,
    );
    expect(renderer.animated).toBe(true);
    renderer.draw(0);
    calls.length = 0;
    renderer.draw(500, false);
    expect(calls).toEqual([
      { image: mask, alpha: 1 },
      { image: wall, alpha: 1 },
    ]);
    expect(amounts.at(-1)).toBe(0.3);
    calls.length = 0;
    renderer.draw(500 + 1000 / 120, false);
    expect(amounts.at(-1)).toBeGreaterThan(0.3);
    expect(calls.at(-1)?.image).toBe(wall);
    expect(calls.some((c) => c.image === distant)).toBe(false);
  } finally {
    vi.unstubAllGlobals();
  }
});
