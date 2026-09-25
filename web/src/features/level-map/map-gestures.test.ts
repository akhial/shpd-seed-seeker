import { describe, expect, it } from "vite-plus/test";
import {
  constrainMapTransform,
  FIT_MAP,
  MapGesture,
  wheelZoomFactor,
  zoomMapAt,
} from "./map-gestures";

const geometry = { width: 500, height: 400, mapWidth: 1000, mapHeight: 1000 };

describe("map gestures", () => {
  it("keeps the layout point beneath the wheel or trackpad cursor fixed while zooming", () => {
    const anchor = { x: 80, y: -40 };
    const next = zoomMapAt(FIT_MAP, 2, anchor, geometry);
    expect(next).toEqual({ zoom: 2, x: -80, y: 40 });
    expect((anchor.x - next.x) / next.zoom).toBe(anchor.x);
    expect((anchor.y - next.y) / next.zoom).toBe(anchor.y);
    expect(zoomMapAt(next, 1, anchor, geometry)).toEqual(FIT_MAP);
  });

  it("starts a real two-pointer pinch directly from the fitted view", () => {
    const gesture = new MapGesture();
    gesture.down(1, { x: -50, y: 0 }, FIT_MAP);
    gesture.down(2, { x: 50, y: 0 }, FIT_MAP);
    gesture.move(1, { x: -100, y: 0 }, geometry);
    expect(gesture.move(2, { x: 100, y: 0 }, geometry)).toEqual({ zoom: 2, x: 0, y: 0 });
  });

  it("transitions from panning to a moving pinch and back to one finger without jumps", () => {
    const gesture = new MapGesture();
    gesture.down(1, { x: -50, y: 0 }, { zoom: 2, x: 0, y: 0 });
    const panned = gesture.move(1, { x: -30, y: 20 }, geometry)!;
    expect(panned).toEqual({ zoom: 2, x: 20, y: 20 });
    gesture.down(2, { x: 70, y: 20 }, panned);
    const pinched = gesture.move(2, { x: 170, y: 20 }, geometry)!;
    expect(pinched).toEqual({ zoom: 4, x: 70, y: 20 });
    gesture.up(1, pinched);
    expect(gesture.move(2, { x: 170, y: 20 }, geometry)).toEqual(pinched);
    expect(gesture.move(2, { x: 180, y: 35 }, geometry)).toEqual({ zoom: 4, x: 80, y: 35 });
    expect(gesture.move(1, { x: 0, y: 0 }, geometry)).toBeUndefined();
  });

  it("pans with a two-finger midpoint while their separation stays constant", () => {
    const gesture = new MapGesture();
    const initial = { zoom: 2, x: 0, y: 0 };
    gesture.down(1, { x: -50, y: 0 }, initial);
    gesture.down(2, { x: 50, y: 0 }, initial);
    gesture.move(1, { x: -30, y: 30 }, geometry);
    expect(gesture.move(2, { x: 70, y: 30 }, geometry)).toEqual({ zoom: 2, x: 20, y: 30 });
  });

  it("rebases after cancellation, extra fingers, and keyboard or wheel changes", () => {
    const gesture = new MapGesture();
    const initial = { zoom: 2, x: 0, y: 0 };
    gesture.down(1, { x: 0, y: 0 }, initial);
    gesture.down(2, { x: 100, y: 0 }, initial);
    gesture.down(3, { x: 200, y: 0 }, initial);
    expect(gesture.move(3, { x: 250, y: 0 }, geometry)).toBeUndefined();
    gesture.up(1, initial);
    gesture.up(1, initial); // A cancel can be followed by lost pointer capture.
    expect(gesture.move(2, { x: 100, y: 0 }, geometry)).toEqual(initial);
    gesture.up(3, initial);
    const zoomed = { zoom: 3, x: 20, y: 30 };
    gesture.rebase(zoomed);
    expect(gesture.move(2, { x: 110, y: 10 }, geometry)).toEqual({ zoom: 3, x: 30, y: 40 });
  });

  it("bounds large pans and recenters a zoom back to fit", () => {
    const bounded = constrainMapTransform({ zoom: 2, x: 9999, y: -9999 }, geometry);
    expect(bounded).toEqual({ zoom: 2, x: 162, y: -212 });
    expect(constrainMapTransform({ ...bounded, zoom: 0.5 }, geometry)).toEqual(FIT_MAP);
    expect(zoomMapAt(FIT_MAP, 100, { x: 0, y: 0 }, geometry).zoom).toBe(6);
    expect(constrainMapTransform(bounded, { ...geometry, width: 900, height: 100 })).toEqual({
      zoom: 2,
      x: 0,
      y: -62,
    });
  });

  it("normalizes wheel units and recognizes the stronger trackpad pinch signal", () => {
    expect(wheelZoomFactor(-1, 1, false)).toBe(wheelZoomFactor(-16, 0, false));
    expect(wheelZoomFactor(-1, 2, false)).toBe(wheelZoomFactor(-350, 0, false));
    expect(wheelZoomFactor(-10, 0, true)).toBeGreaterThan(wheelZoomFactor(-10, 0, false));
    expect(wheelZoomFactor(10, 0, false)).toBeLessThan(1);
    expect(wheelZoomFactor(0, 0, false)).toBe(1);
  });
});
