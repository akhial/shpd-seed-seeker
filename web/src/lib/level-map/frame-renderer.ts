import { createMapSceneRenderer } from "./scene-renderer";
import type { RenderRequest, RenderResponse } from "./render-worker";
import type { MapBundle } from "./types";

/** At most one frame is in flight; slow devices never accumulate render jobs. */
export function createMapFrameRenderer(
  context: CanvasRenderingContext2D,
  particleContext: CanvasRenderingContext2D,
  bundle: MapBundle,
  revealSecrets: boolean,
) {
  let worker: Worker | undefined;
  let fallback: ReturnType<typeof createMapSceneRenderer> | undefined;
  let disposed = false;
  let pending:
    | {
        elapsed: number;
        advanceSprites: boolean;
        density: number;
        resolve: (animated: boolean) => void;
        promise: Promise<boolean>;
      }
    | undefined;
  const drawFallback = (elapsed: number, advanceSprites: boolean, density: number) => {
    fallback ??= createMapSceneRenderer(context, particleContext, bundle, revealSecrets);
    fallback.draw(elapsed, advanceSprites, density);
    return fallback.animated;
  };
  const failover = () => {
    worker?.terminate();
    worker = undefined;
    if (pending) {
      const frame = pending;
      pending = undefined;
      frame.resolve(!disposed && drawFallback(frame.elapsed, frame.advanceSprites, frame.density));
    }
  };
  if (typeof Worker !== "undefined" && typeof OffscreenCanvas !== "undefined") {
    try {
      worker = new Worker(new URL("./render-worker.ts", import.meta.url), { type: "module" });
      worker.onerror = failover;
      worker.onmessageerror = failover;
      worker.onmessage = ({ data }: MessageEvent<RenderResponse>) => {
        if (data.kind === "error") {
          failover();
          return;
        }
        if (!disposed && pending) {
          context.drawImage(data.scenery, 0, 0);
          const canvas = particleContext.canvas;
          if (canvas.width !== data.effects.width || canvas.height !== data.effects.height) {
            canvas.width = data.effects.width;
            canvas.height = data.effects.height;
          }
          // Replace transparent pixels too, so old particles cannot leave trails.
          particleContext.globalCompositeOperation = "copy";
          particleContext.drawImage(data.effects, 0, 0);
          particleContext.globalCompositeOperation = "source-over";
          pending.resolve(data.animated);
          pending = undefined;
        }
        data.scenery.close();
        data.effects.close();
      };
      // Clone the bitmap handles: the map cache still owns and uses its textures.
      worker.postMessage({ kind: "init", bundle, revealSecrets } satisfies RenderRequest);
    } catch {
      failover();
    }
  }
  return {
    draw(elapsed: number, advanceSprites = true, density = 1): Promise<boolean> {
      if (disposed) return Promise.resolve(false);
      if (pending) return pending.promise;
      if (!worker) return Promise.resolve(drawFallback(elapsed, advanceSprites, density));
      let resolve!: (animated: boolean) => void;
      const promise = new Promise<boolean>((done) => {
        resolve = done;
      });
      pending = { elapsed, advanceSprites, density, resolve, promise };
      try {
        worker.postMessage({
          kind: "draw",
          elapsed,
          advanceSprites,
          density,
        } satisfies RenderRequest);
      } catch {
        failover();
      }
      return promise;
    },
    dispose() {
      disposed = true;
      worker?.terminate();
      worker = undefined;
      pending?.resolve(false);
      pending = undefined;
      fallback = undefined;
    },
  };
}
