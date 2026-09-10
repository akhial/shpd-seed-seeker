import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it } from "vite-plus/test";
import init, { level_map, level_map_asset } from "../wasm/pkg/seedfinder.js";
import { isMapDepthSupported, mapRequestJson } from "./client";
import { drawLevelMap } from "./render";
import type { LevelMapDocument, MapBundle } from "./types";

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
  it("uses engine coverage, including quest parent floors but excluding boss arenas", () => {
    expect([1, 13, 19, 24].every(isMapDepthSupported)).toBe(true);
    expect([0, 5, 10, 15, 20, 25, 26].some(isMapDepthSupported)).toBe(false);
  });
  it("canonicalizes challenge order while isolating trinket, floor and branch cache entries", () => {
    const request = { seed: "AAA-AAA-AAA", depth: 13, challenges: ["no_armor", "no_food"] };
    const key = mapRequestJson(request);
    expect(mapRequestJson({ ...request, challenges: ["no_food", "no_armor", "no_food"] })).toBe(
      key,
    );
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

it("composites layers at cell origins, advances animation and adds secrets only on request", () => {
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
      entrance: null,
      exit: null,
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
  expect(blits).toEqual([
    [texture, 16, 0, 16, 16, 16, 0, 16, 16],
    [texture, 32, 0, 8, 8, 20, 2, 8, 8],
  ]);
  expect(outlines).toEqual([]);
  blits.length = 0;
  drawLevelMap(context, bundle, 400, true);
  expect(blits[0]).toEqual([texture, 0, 0, 16, 16, 16, 0, 16, 16]);
  expect(outlines).toEqual([[17, 1, 14, 14]]);
  expect(context.imageSmoothingEnabled).toBe(false);
});
