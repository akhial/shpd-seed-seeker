import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { createMapFrameRenderer } from "./frame-renderer";
import { createMapSceneRenderer } from "./scene-renderer";
import type { MapBundle } from "../types";
import type { RenderResponse } from "./render-worker";

vi.mock("./scene-renderer", () => ({ createMapSceneRenderer: vi.fn() }));

class RenderWorker {
  static current: RenderWorker;
  postMessage = vi.fn();
  terminate = vi.fn();
  onmessage?: (event: { data: RenderResponse }) => void;
  onerror?: () => void;
  onmessageerror?: () => void;
  constructor() {
    RenderWorker.current = this;
  }
  frame(animated = true) {
    const scenery = { close: vi.fn() };
    const effects = { close: vi.fn(), width: 96, height: 64 };
    this.onmessage?.({
      data: {
        kind: "frame",
        animated,
        scenery: scenery as unknown as ImageBitmap,
        effects: effects as unknown as ImageBitmap,
      },
    });
    return { scenery, effects };
  }
}
const bundle = { textures: new Map() } as unknown as MapBundle;
const drawImage = vi.fn();
const context = { drawImage } as unknown as CanvasRenderingContext2D;
const drawEffects = vi.fn();
const particleContext = {
  canvas: { width: 1, height: 1 },
  drawImage: drawEffects,
} as unknown as CanvasRenderingContext2D;
const fallback = { animated: true, draw: vi.fn() };

beforeEach(() => {
  vi.stubGlobal("Worker", RenderWorker);
  vi.stubGlobal("OffscreenCanvas", class {});
  vi.mocked(createMapSceneRenderer).mockReturnValue(fallback);
});
afterEach(() => {
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

it("renders off the UI thread, bounds in-flight work and releases presented bitmaps", async () => {
  const renderer = createMapFrameRenderer(context, particleContext, bundle, true);
  const worker = RenderWorker.current;
  expect(worker.postMessage).toHaveBeenCalledExactlyOnceWith({
    kind: "init",
    bundle,
    revealSecrets: true,
  });
  const first = renderer.draw(100);
  expect(renderer.draw(200)).toBe(first);
  expect(worker.postMessage).toHaveBeenCalledTimes(2);
  expect(createMapSceneRenderer).not.toHaveBeenCalled();
  expect(drawImage).not.toHaveBeenCalled();
  const bitmap = worker.frame();
  expect(await first).toBe(true);
  expect(drawImage).toHaveBeenCalledExactlyOnceWith(bitmap.scenery, 0, 0);
  expect(bitmap.scenery.close).toHaveBeenCalledOnce();
  expect(bitmap.effects.close).toHaveBeenCalledOnce();
  expect(drawEffects).toHaveBeenCalledExactlyOnceWith(bitmap.effects, 0, 0);
  expect(particleContext.canvas.width).toBe(96);
  expect(particleContext.canvas.height).toBe(64);
  const next = renderer.draw(300, false, 2.5);
  expect(worker.postMessage).toHaveBeenLastCalledWith({
    kind: "draw",
    elapsed: 300,
    advanceSprites: false,
    density: 2.5,
  });
  worker.frame(false);
  expect(await next).toBe(false);
  renderer.dispose();
  expect(worker.terminate).toHaveBeenCalledOnce();
});

it("cancels a slow frame on floor changes and discards a late bitmap", async () => {
  const renderer = createMapFrameRenderer(context, particleContext, bundle, false);
  const worker = RenderWorker.current;
  const pending = renderer.draw(50);
  renderer.dispose();
  expect(await pending).toBe(false);
  const bitmap = worker.frame();
  expect(bitmap.scenery.close).toHaveBeenCalledOnce();
  expect(bitmap.effects.close).toHaveBeenCalledOnce();
  expect(drawImage).not.toHaveBeenCalled();
  expect(await renderer.draw(100)).toBe(false);
  expect(worker.postMessage).toHaveBeenCalledTimes(2);
});

it.each(["error", "messageerror", "unsupported"])(
  "falls back if the worker reports %s",
  async (failure) => {
    const renderer = createMapFrameRenderer(context, particleContext, bundle, true);
    const worker = RenderWorker.current;
    const pending = renderer.draw(75);
    if (failure === "error") worker.onerror!();
    else if (failure === "messageerror") worker.onmessageerror!();
    else worker.onmessage!({ data: { kind: "error" } });
    expect(await pending).toBe(true);
    expect(worker.terminate).toHaveBeenCalledOnce();
    expect(createMapSceneRenderer).toHaveBeenCalledExactlyOnceWith(
      context,
      particleContext,
      bundle,
      true,
    );
    expect(fallback.draw).toHaveBeenCalledExactlyOnceWith(75, true, 1);
    await renderer.draw(125);
    expect(createMapSceneRenderer).toHaveBeenCalledOnce();
    expect(fallback.draw).toHaveBeenLastCalledWith(125, true, 1);
    renderer.dispose();
  },
);

it.each(["missing", "startup", "clone"])(
  "supports browsers with %s worker support",
  async (failure) => {
    if (failure === "missing") vi.stubGlobal("OffscreenCanvas", undefined);
    else if (failure === "startup")
      vi.stubGlobal(
        "Worker",
        class {
          constructor() {
            throw new Error("blocked");
          }
        },
      );
    const renderer = createMapFrameRenderer(context, particleContext, bundle, false);
    if (failure === "clone")
      RenderWorker.current.postMessage.mockImplementation(() => {
        throw new Error("clone");
      });
    expect(await renderer.draw(10)).toBe(true);
    expect(fallback.draw).toHaveBeenCalledExactlyOnceWith(10, true, 1);
    renderer.dispose();
  },
);
