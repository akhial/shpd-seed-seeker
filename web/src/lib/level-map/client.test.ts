import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { LevelMapDocument, MapWorkerRequest, MapWorkerResponse } from "./types";

class MapWorker {
  static instances: MapWorker[] = [];
  onmessage?: (event: MessageEvent<MapWorkerResponse>) => Promise<void>;
  onerror?: () => void;
  onmessageerror?: () => void;
  postMessage = vi.fn<(request: MapWorkerRequest) => void>();
  terminate = vi.fn();
  constructor() {
    MapWorker.instances.push(this);
  }
  reply(response: MapWorkerResponse) {
    return this.onmessage?.({ data: response } as MessageEvent<MapWorkerResponse>);
  }
}

const input = { seed: "AAA-AAA-AAA", depth: 1, challenges: [] };
const map: LevelMapDocument = {
  format: "seed-seeker-level-map",
  schemaVersion: 2,
  seed: input.seed,
  depth: 1,
  branch: 0,
  kind: "regular",
  challenges: [],
  selectedTrinket: null,
  width: 1,
  height: 1,
  terrain: [0],
  entrance: null,
  exit: null,
  secretRooms: [],
  secretDoors: [],
  secretTraps: [],
  traps: [],
  branches: [],
  assets: [{ id: "tiles", width: 16, height: 16, sha256: "tile-revision" }],
  scene: { tileSize: 16, sprites: [], layers: [], concealedLayers: [] },
};
const assets = [{ id: "tiles", png: new ArrayBuffer(0) }];

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  MapWorker.instances = [];
  vi.stubGlobal("Worker", MapWorker);
  vi.stubGlobal("createImageBitmap", vi.fn().mockResolvedValue({}));
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("map loading", () => {
  it("shares pending requests and decoded textures, and bounds the map cache", async () => {
    const { requestLevelMap } = await import("./client");
    const first = requestLevelMap(input);
    expect(requestLevelMap(input)).toBe(first);
    const worker = MapWorker.instances[0];
    await worker.reply({ id: 1, map, assets });
    await first;
    expect(requestLevelMap(input)).toBe(first);
    for (let depth = 2; depth <= 9; depth++) {
      const request = requestLevelMap({ ...input, depth });
      await worker.reply({ id: depth, map: { ...map, depth }, assets });
      await request;
    }
    expect(createImageBitmap).toHaveBeenCalledTimes(1);
    const reloaded = requestLevelMap(input);
    expect(reloaded).not.toBe(first);
    await worker.reply({ id: 10, map, assets });
    await reloaded;
    expect(MapWorker.instances).toHaveLength(1);
  });

  it("retries after an asset cannot be decoded", async () => {
    vi.mocked(createImageBitmap).mockRejectedValueOnce(new Error("Bad PNG"));
    const { requestLevelMap } = await import("./client");
    const failure = expect(requestLevelMap(input)).rejects.toThrow("Bad PNG");
    const worker = MapWorker.instances[0];
    await worker.reply({ id: 1, map, assets });
    await failure;
    const retry = requestLevelMap(input);
    await worker.reply({ id: 2, map, assets });
    expect((await retry).map).toBe(map);
    expect(createImageBitmap).toHaveBeenCalledTimes(2);
  });

  it.each(["timeout", "onerror", "onmessageerror"] as const)(
    "rejects pending requests after %s and starts a fresh worker on retry",
    async (failure) => {
      const { requestLevelMap } = await import("./client");
      const first = expect(requestLevelMap(input)).rejects.toThrow();
      const second = expect(requestLevelMap({ ...input, depth: 2 })).rejects.toThrow();
      const worker = MapWorker.instances[0];
      if (failure === "timeout") await vi.advanceTimersByTimeAsync(30_000);
      else worker[failure]?.();
      await Promise.all([first, second]);
      expect(worker.terminate).toHaveBeenCalledTimes(1);
      expect(vi.getTimerCount()).toBe(0);
      const retry = requestLevelMap(input);
      expect(MapWorker.instances).toHaveLength(2);
      await MapWorker.instances[1].reply({ id: 3, map, assets });
      expect((await retry).map).toBe(map);
    },
  );

  it("cleans up a failed send without timing out another request", async () => {
    const { requestLevelMap } = await import("./client");
    const first = requestLevelMap(input);
    const worker = MapWorker.instances[0];
    worker.postMessage.mockImplementationOnce(() => {
      throw new Error("Cannot send");
    });
    await expect(requestLevelMap({ ...input, depth: 2 })).rejects.toThrow("Cannot send");
    await worker.reply({ id: 1, map, assets });
    await first;
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(worker.terminate).not.toHaveBeenCalled();
  });
});
