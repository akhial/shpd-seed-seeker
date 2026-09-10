import init, { level_map, level_map_asset } from "../wasm/pkg/seedfinder.js";
import type { LevelMapDocument, MapWorkerRequest, MapWorkerResponse } from "./types";

// Map replay has a dedicated worker so opening a map cannot stall the scout or search UI.
const ready = init({ module_or_path: new URL("../wasm/pkg/seedfinder_bg.wasm", import.meta.url) });
self.onmessage = async ({ data }: MessageEvent<MapWorkerRequest>) => {
  try {
    await ready;
    const map = JSON.parse(level_map(data.requestJson)) as LevelMapDocument;
    if (map.schemaVersion !== 1 || map.format !== "seed-seeker-level-map") {
      throw new Error("This map needs a newer version of Seed Seeker.");
    }
    const assets = map.assets.map(({ id }) => ({ id, png: level_map_asset(id).slice().buffer }));
    const response: MapWorkerResponse = { id: data.id, map, assets };
    self.postMessage(response, { transfer: assets.map(({ png }) => png) });
  } catch (error) {
    const response: MapWorkerResponse = {
      id: data.id,
      error: error instanceof Error ? error.message : String(error),
    };
    self.postMessage(response);
  }
};
