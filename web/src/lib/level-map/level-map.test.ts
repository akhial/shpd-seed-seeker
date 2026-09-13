import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it, vi } from "vite-plus/test";
import init, { level_map, level_map_asset } from "../wasm/pkg/seedfinder.js";
import { isMapDepthSupported, mapRequestJson } from "./client";
import { mapSpriteCache } from "./frame-cache";
import { createLevelMapRenderer, drawLevelMap, glowAmount } from "./render";
import { itemGlow } from "../glow";
import { particleState } from "./particles";
import type { LevelMapDocument, LevelMapRequest, MapBundle } from "./types";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("../wasm/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});
const map = (depth: number, branch = 0) =>
  JSON.parse(
    level_map(mapRequestJson({ seed: "AAA-AAA-AAA", depth, branch, challenges: [] })),
  ) as LevelMapDocument;

describe("browser level map contract", () => {
  it("alternates Vault warnings instead of lighting every vent at once", () => {
    const result = JSON.parse(
      level_map(JSON.stringify({ seed: "HEL-LOO-WRD", depth: 18, branch: 1 })),
    ) as LevelMapDocument;
    const vents = result.contents!.features.filter((f) => f.cycle);
    const cells = (cooldown: number) => vents.filter((f) => f.cycle!.cooldown === cooldown);
    const warnings = (cooldown: number, time: number) =>
      cells(cooldown)
        .filter((f) => {
          const emitter = result.scene.emitters!.find((e) => e.cell === f.cell)!;
          return emitter.particles.some((p) => particleState(emitter, p, time));
        })
        .map((f) => f.cell);
    const even = warnings(2, 700),
      odd = warnings(2, 1700);
    expect(even.length).toBeGreaterThan(0);
    expect(odd.length).toBeGreaterThan(0);
    expect(even.some((cell) => odd.includes(cell))).toBe(false);
    expect(even.length + odd.length).toBe(cells(2).length);
    expect(warnings(2, 2700)).toEqual(even);
    const path = warnings(5, 20700);
    expect(path.length).toBeGreaterThan(0);
    expect(path.length).toBeLessThan(cells(5).length);
    expect(warnings(5, 21700)).not.toEqual(path);
    expect(warnings(1, 20700)).toHaveLength(cells(1).length);
    expect(warnings(1, 21700)).toHaveLength(cells(1).length);
  });
  it("carries the same enchantment glows as the Scout list through WASM", () => {
    const result = JSON.parse(
      level_map(JSON.stringify({ seed: "FOI-QDX-EMJ", depth: 22, trinket: "parchment_scrap" })),
    ) as LevelMapDocument;
    for (const [cell, name] of [
      [1518, "Unstable"],
      [1660, "Blocking"],
    ] as const) {
      const glow = result.contents!.heaps.find((h) => h.cell === cell)!.items[0].glow!;
      const scout = itemGlow({ cursed: false, effect: { kind: "enchantment", name } })!;
      expect(`#${glow.color.map((c) => c.toString(16).padStart(2, "0")).join("")}`).toBe(
        scout.color,
      );
      expect(glow.periodMs).toBe(scout.period * 1000);
    }
  });
  it("pulses continuously to the game's 60% peak at display-rate timestamps", () => {
    expect(glowAmount(1000, 0)).toBe(0);
    expect(glowAmount(1000, 500)).toBe(0.3);
    expect(glowAmount(1000, 1000)).toBe(0.6);
    expect(glowAmount(1000, 1500)).toBe(0.3);
    expect(glowAmount(1000, 2000)).toBe(0);
    expect(glowAmount(500, 500)).toBe(0.6);
    expect(glowAmount(1000, 500 + 1000 / 120)).toBeGreaterThan(glowAmount(1000, 500));
  });
  it("uses engine coverage, including quest parent floors and supported boss arenas", () => {
    expect([1, 5, 13, 15, 19, 24].every(isMapDepthSupported)).toBe(true);
    expect([0, 10, 20, 25, 26].some(isMapDepthSupported)).toBe(false);
  });
  it("canonicalizes challenge order while isolating trinket, floor and branch cache entries", () => {
    const request: LevelMapRequest = {
      seed: "AAA-AAA-AAA",
      depth: 13,
      challenges: ["faith_is_my_armor", "on_diet"],
    };
    const key = mapRequestJson(request);
    expect(
      mapRequestJson({ ...request, challenges: ["on_diet", "faith_is_my_armor", "on_diet"] }),
    ).toBe(key);
    expect(JSON.parse(key).trinket).toBe("none");
    expect(mapRequestJson({ ...request, selectedTrinket: "mossy_clump" })).not.toBe(key);
    expect(mapRequestJson({ ...request, branch: 1 })).not.toBe(key);
    expect(mapRequestJson({ ...request, depth: 14 })).not.toBe(key);
  });
  it.each([
    [13, "blacksmith_crystal"],
    [19, "imp_vault"],
  ] as const)("follows the real quest branch on floor %i", (depth, kind) => {
    const parent = map(depth);
    expect(parent.branches).toEqual([expect.objectContaining({ depth, branch: 1, kind })]);
    const quest = map(depth, 1);
    expect(quest.kind).toBe(kind);
    expect(quest.exit).toBeNull();
    expect(quest.terrain).toHaveLength(quest.width * quest.height);
    expect(quest.branches).toEqual([]);
  });
  it("loads original PNGs for every sprite in the engine scene", () => {
    const scene = map(1);
    for (const asset of scene.assets) {
      expect(level_map_asset(asset.id).slice(0, 8)).toEqual(
        new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
      );
    }
    const ids = new Set(scene.assets.map((asset) => asset.id));
    expect(scene.scene.sprites.some((sprite) => sprite.frames.length > 1)).toBe(true);
    for (const sprite of scene.scene.sprites)
      for (const frame of sprite.frames)
        for (const command of frame) {
          if (command.kind === "blit") expect(ids.has(command.asset)).toBe(true);
        }
  });
});

it("composites raised layers and switches secret visibility without drawing annotations", () => {
  const floor = map(1);
  const texture = {} as ImageBitmap;
  const blits: unknown[][] = [];
  const outlines: unknown[][] = [];
  const context = {
    clearRect() {},
    fillRect() {},
    beginPath() {},
    arc() {},
    stroke() {},
    drawImage(...args: unknown[]) {
      blits.push(args);
    },
    strokeRect(...args: unknown[]) {
      outlines.push(args);
    },
  } as unknown as CanvasRenderingContext2D;
  const bundle: MapBundle = {
    textures: new Map([["tiles", texture]]),
    map: {
      ...floor,
      width: 2,
      height: 2,
      entrance: 0,
      exit: 3,
      branches: [],
      secretRooms: [],
      secretDoors: [1],
      secretTraps: [],
      scene: {
        tileSize: 16,
        layers: [
          { name: "terrain", cells: [null, 0, null, null] },
          { name: "features", cells: [null, 1, null, null] },
        ],
        concealedLayers: [{ name: "terrain", cells: [null, 0, null, null] }],
        sprites: [
          {
            frameDurationMs: 200,
            frames: [
              [
                {
                  kind: "blit",
                  asset: "tiles",
                  source: [0, 0, 16, 16],
                  destination: [0, 0, 16, 16],
                },
              ],
              [
                {
                  kind: "blit",
                  asset: "tiles",
                  source: [16, 0, 16, 16],
                  destination: [0, 0, 16, 16],
                },
              ],
            ],
          },
          {
            frameDurationMs: 1,
            frames: [
              [{ kind: "blit", asset: "tiles", source: [32, 0, 8, 8], destination: [4, 2, 8, 8] }],
            ],
          },
        ],
      },
    },
  };
  drawLevelMap(context, bundle, 200, false);
  expect(blits).toEqual([[texture, 16, 0, 16, 16, 16, 0, 16, 16]]);
  expect(outlines).toEqual([]);
  blits.length = 0;
  drawLevelMap(context, bundle, 400, true);
  expect(blits[0]).toEqual([texture, 0, 0, 16, 16, 16, 0, 16, 16]);
  expect(blits[1]).toEqual([texture, 32, 0, 8, 8, 20, 2, 8, 8]);
  expect(outlines).toEqual([]);
  expect(context.imageSmoothingEnabled).toBe(false);
});

it("caches animation states and repairs overlapping layers without repainting distant tiles", () => {
  const floor = map(1);
  const fill = (rgba: [number, number, number, number]) => ({
    kind: "fill" as const,
    rgba,
    destination: [0, 0, 16, 16] as [number, number, number, number],
  });
  const bundle: MapBundle = {
    textures: new Map(),
    map: {
      ...floor,
      width: 4,
      height: 1,
      scene: {
        tileSize: 16,
        sprites: [
          { frameDurationMs: 1, frames: [[fill([100, 100, 100, 255])]] },
          { frameDurationMs: 50, frames: [[fill([255, 0, 0, 255])], []] },
          { frameDurationMs: 1, frames: [[fill([0, 0, 255, 128])]] },
        ],
        layers: [
          { name: "terrain", cells: [0, null, null, 0] },
          { name: "animation", cells: [1, null, null, null] },
          { name: "foreground", cells: [2, null, null, null] },
        ],
        concealedLayers: [{ name: "terrain", cells: [0, null, null, 0] }],
      },
    },
  };
  const raster = vi.fn();
  const canvases: HTMLCanvasElement[] = [];
  const makeCanvas = () => {
    const canvas = { getContext: () => ({ fillRect: raster }) } as unknown as HTMLCanvasElement;
    canvases.push(canvas);
    return canvas;
  };
  const blit = vi.fn();
  const rect = vi.fn();
  const fillRect = vi.fn();
  const context = {
    save() {},
    restore() {},
    beginPath() {},
    clip() {},
    rect,
    fillRect,
    drawImage: blit,
  } as unknown as CanvasRenderingContext2D;
  const renderer = createLevelMapRenderer(context, bundle, true, makeCanvas);
  expect(renderer.animated).toBe(true);
  expect(canvases.length).toBeLessThan(8); // Shared water atlas plus tinted actor silhouettes. // All unique composite frames share one atlas.
  expect(raster).toHaveBeenCalledTimes(3); // The transparent animation frame has no commands.
  renderer.draw(0);
  expect(blit).toHaveBeenCalledTimes(4);
  blit.mockClear();
  fillRect.mockClear();
  renderer.draw(25);
  expect(blit).not.toHaveBeenCalled();
  expect(fillRect).not.toHaveBeenCalled();
  renderer.draw(50);
  expect(rect).toHaveBeenCalledWith(0, 0, 16, 16);
  expect(blit.mock.calls).toEqual([
    [canvases[0], 0, 0, 16, 16, 0, 0, 16, 16], // Restore terrain under the empty frame.
    [canvases[0], 32, 0, 16, 16, 0, 0, 16, 16], // Preserve the foreground.
  ]);
  blit.mockClear();
  renderer.draw(100);
  expect(blit.mock.calls[1]).toEqual([canvases[0], 16, 0, 16, 16, 0, 0, 16, 16]);
  expect(raster).toHaveBeenCalledTimes(3); // Looping never rasterizes a frame again.
  const concealed = createLevelMapRenderer(context, bundle, false, makeCanvas);
  expect(concealed.animated).toBe(false);
  concealed.draw(0);
  blit.mockClear();
  concealed.draw(1000);
  expect(blit).not.toHaveBeenCalled();
});

it("shares phase-shifted water frames and uses source textures for single blits", () => {
  const floor = map(1);
  const textures = new Map(floor.assets.map(({ id }) => [id, {} as ImageBitmap]));
  const canvases: HTMLCanvasElement[] = [];
  const sprites = mapSpriteCache({ map: floor, textures }, () => {
    const canvas = {
      getContext: () => ({ fillRect() {}, drawImage() {} }),
    } as unknown as HTMLCanvasElement;
    canvases.push(canvas);
    return canvas;
  });
  const water = floor.scene.layers.find((layer) => layer.name === "water")!;
  const phases = [...new Set(water.cells.filter((index) => index !== null))];
  expect(phases).toHaveLength(4);
  const frames = phases.flatMap((index) => sprites[index].frames);
  expect(frames).toHaveLength(128);
  expect(new Set(frames).size).toBe(64);
  expect(canvases.length).toBeLessThan(8); // Shared water atlas plus tinted actor silhouettes.
  for (const [index, sprite] of floor.scene.sprites.entries()) {
    for (const [frameIndex, commands] of sprite.frames.entries()) {
      if (commands.length === 1 && commands[0].kind === "blit" && !commands[0].tint) {
        expect(sprites[index].frames[frameIndex]?.image).toBe(textures.get(commands[0].asset));
      }
    }
  }
});

it("reuses the same map atlas when changing secret visibility or reopening a map", () => {
  const floor = map(1);
  const bundle: MapBundle = {
    map: floor,
    textures: new Map(floor.assets.map(({ id }) => [id, {} as ImageBitmap])),
  };
  const createElement = vi.fn(() => ({
    getContext: () => ({ fillRect() {}, drawImage() {} }),
  }));
  vi.stubGlobal("document", { createElement });
  try {
    const first = mapSpriteCache(bundle);
    const count = createElement.mock.calls.length;
    expect(mapSpriteCache(bundle)).toBe(first);
    expect(createElement).toHaveBeenCalledTimes(count);
  } finally {
    vi.unstubAllGlobals();
  }
});
