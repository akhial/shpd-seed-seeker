/** The same renderer runs in a worker and in the compatibility fallback. */
export type MapCanvas = HTMLCanvasElement | OffscreenCanvas;
export type MapContext = CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D;

export function makeMapCanvas(): MapCanvas {
  return typeof document === "undefined"
    ? new OffscreenCanvas(1, 1)
    : document.createElement("canvas");
}

export function mapContext(canvas: MapCanvas): MapContext {
  return canvas.getContext("2d") as MapContext;
}
