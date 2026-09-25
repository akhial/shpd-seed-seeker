import { createMapSceneRenderer } from "./scene-renderer";
import type { MapBundle } from "../types";

export type RenderRequest =
  | { kind: "init"; bundle: MapBundle; revealSecrets: boolean }
  | { kind: "draw"; elapsed: number; advanceSprites: boolean; density: number };
export type RenderResponse =
  | { kind: "frame"; scenery: ImageBitmap; effects: ImageBitmap; animated: boolean }
  | { kind: "error" };

let scenery: OffscreenCanvas;
let effects: OffscreenCanvas;
let renderer: ReturnType<typeof createMapSceneRenderer>;
self.onmessage = async ({ data }: MessageEvent<RenderRequest>) => {
  try {
    if (data.kind === "init") {
      const { map } = data.bundle;
      scenery = new OffscreenCanvas(
        map.width * map.scene.tileSize,
        map.height * map.scene.tileSize,
      );
      effects = new OffscreenCanvas(1, 1);
      renderer = createMapSceneRenderer(
        scenery.getContext("2d")!,
        effects.getContext("2d")!,
        data.bundle,
        data.revealSecrets,
      );
    } else {
      renderer.draw(data.elapsed, data.advanceSprites, data.density);
      // Snapshots retain both surfaces for incremental updates. Keep particles
      // separate: compositing at scenery resolution loses subpixel detail.
      const background = await createImageBitmap(scenery);
      let overlay: ImageBitmap;
      try {
        overlay = await createImageBitmap(effects);
      } catch (error) {
        background.close();
        throw error;
      }
      self.postMessage(
        { kind: "frame", scenery: background, effects: overlay, animated: renderer.animated },
        { transfer: [background, overlay] },
      );
    }
  } catch {
    self.postMessage({ kind: "error" });
  }
};
