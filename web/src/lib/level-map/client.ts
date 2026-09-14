import { engine_info } from "../wasm/pkg/seedfinder.js";
import type { LevelMapRequest, MapBundle, MapWorkerResponse } from "./types";

let supported: Set<number> | undefined;
/** The scout is available only after engine initialization. Coverage comes from the engine. */
export function isMapDepthSupported(depth: number): boolean {
  supported ??= new Set<number>(JSON.parse(engine_info()).levelMaps.supportedDepths);
  return supported.has(depth);
}
export function mapRequestJson(request: LevelMapRequest): string {
  return JSON.stringify({
    seed: request.seed,
    depth: request.depth,
    branch: request.branch ?? 0,
    challenges: [...new Set(request.challenges)].sort(),
    trinket: request.selectedTrinket ?? "none",
  });
}

let worker: Worker | undefined;
let nextId = 0;
const pending = new Map<
  number,
  {
    resolve: (bundle: MapBundle) => void;
    reject: (error: Error) => void;
    timeout: ReturnType<typeof setTimeout>;
  }
>();
const cache = new Map<string, Promise<MapBundle>>();
const textures = new Map<string, Promise<ImageBitmap>>();

function resetWorker(error: Error) {
  worker?.terminate();
  worker = undefined;
  for (const request of pending.values()) {
    clearTimeout(request.timeout);
    request.reject(error);
  }
  pending.clear();
  cache.clear();
}
function getWorker(): Worker {
  if (worker) return worker;
  worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  worker.onerror = () =>
    resetWorker(new Error("The map engine stopped. Try opening the map again."));
  worker.onmessageerror = () =>
    resetWorker(new Error("The map engine returned unreadable data. Please try again."));
  worker.onmessage = async ({ data }: MessageEvent<MapWorkerResponse>) => {
    const request = pending.get(data.id);
    if (!request) return;
    pending.delete(data.id);
    clearTimeout(request.timeout);
    if ("error" in data) {
      request.reject(new Error(data.error));
      return;
    }
    try {
      const images = await Promise.all(
        data.assets.map(async ({ id, png }) => {
          const revision = data.map.assets.find((asset) => asset.id === id)!.sha256;
          if (!textures.has(revision)) {
            const decoded = createImageBitmap(new Blob([png], { type: "image/png" }));
            textures.set(revision, decoded);
            void decoded.catch(() => textures.delete(revision));
          }
          return [id, await textures.get(revision)!] as const;
        }),
      );
      request.resolve({ map: data.map, textures: new Map(images) });
    } catch (error) {
      request.reject(error instanceof Error ? error : new Error(String(error)));
    }
  };
  return worker;
}
export function requestLevelMap(input: LevelMapRequest): Promise<MapBundle> {
  const key = mapRequestJson(input);
  const existing = cache.get(key);
  if (existing) {
    cache.delete(key);
    cache.set(key, existing);
    return existing;
  }
  const promise = new Promise<MapBundle>((resolve, reject) => {
    const activeWorker = getWorker();
    const id = ++nextId;
    const timeout = setTimeout(
      () => resetWorker(new Error("The map took too long. Please try again.")),
      30_000,
    );
    pending.set(id, { resolve, reject, timeout });
    try {
      activeWorker.postMessage({ id, requestJson: key });
    } catch (error) {
      clearTimeout(timeout);
      pending.delete(id);
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
  cache.set(key, promise);
  while (cache.size > 8) cache.delete(cache.keys().next().value!);
  void promise.catch(() => {
    if (cache.get(key) === promise) cache.delete(key);
  });
  return promise;
}
