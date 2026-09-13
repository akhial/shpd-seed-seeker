import { describe, expect, it, vi } from "vite-plus/test";
import { createMapParticleRenderer, curveValue, particleState } from "./particles";
import type { MapBundle, MapEmitter } from "./types";

const emitter: MapEmitter = {
  wallMask: true,
  cell: 0,
  loopMs: 3000,
  blend: "add",
  image: { kind: "fill", rgba: [34, 238, 102, 255], destination: [0, 0, 1, 1] },
  velocity: [0, 0],
  acceleration: [0, -80],
  angularSpeed: 0,
  alpha: {
    points: [
      [0, 0],
      [200, 1000],
      [1000, 1000],
    ],
    sqrt: false,
  },
  scale: {
    points: [
      [0, 1000],
      [1000, 0],
    ],
    sqrt: false,
  },
  particles: [{ birthMs: 0, lifespanMs: 600, position: [8000, 8000], scale: 4000, angle: 0 }],
};

describe("continuous map effects", () => {
  it("starts scheduled hazards on their captured turn without prewarming future particles", () => {
    const scheduled = { ...emitter, startMs: 2000, loopMs: 5000 };
    const particle = { ...emitter.particles[0], birthMs: 1000 };
    expect(particleState(scheduled, particle, 100)).toBeNull();
    expect(particleState(scheduled, particle, 2999)).toBeNull();
    const first = particleState(scheduled, particle, 3100)!;
    expect(first.y).toBeCloseTo(7.6);
    expect(particleState(scheduled, particle, 3100 + 1000 / 120)!.y).toBeLessThan(first.y);
    expect(particleState(scheduled, particle, 3700)).toBeNull();
    expect(particleState(scheduled, particle, 8100)).toEqual(first);
  });
  it("moves at consecutive 120 Hz timestamps without sprite-frame quantization", () => {
    const particle = emitter.particles[0];
    const first = particleState(emitter, particle, 100)!;
    const next = particleState(emitter, particle, 100 + 1000 / 120)!;
    expect(first.y).toBeCloseTo(7.6);
    expect(next.y).toBeLessThan(first.y);
    expect(next.scale).toBeLessThan(first.scale);
    expect(particleState(emitter, particle, 600)).toBeNull();
    expect(particleState(emitter, particle, 3100)).toEqual(first);
  });
  it("uses the game's nonlinear question pulse and prewarms staggered emitters", () => {
    const pulse = {
      points: [
        [0, 0],
        [500, 4500],
        [1000, 0],
      ] as [number, number][],
      sqrt: true,
    };
    expect(curveValue(pulse, 0.5)).toBeCloseTo(3 * Math.sqrt(0.5));
    expect(curveValue(pulse, 0.25)).toBeCloseTo(curveValue(pulse, 0.75));
    expect(particleState(emitter, { ...emitter.particles[0], birthMs: 2900 }, 0)?.y).toBeCloseTo(
      7.6,
    );
  });
  it("composites additive particles on scenery and applies the selected concealment mask", () => {
    const operations: string[] = [];
    const context = {
      canvas: { width: 64, height: 64 },
      save() {},
      restore() {},
      setTransform() {},
      clearRect() {},
      drawImage: vi.fn(),
      translate: vi.fn(),
      rotate() {},
      scale() {},
      fillRect: vi.fn(),
      set globalCompositeOperation(value: string) {
        operations.push(value);
      },
    };
    vi.stubGlobal("document", { createElement: () => ({ getContext: () => ({ fillRect() {} }) }) });
    const bundle = {
      map: {
        width: 2,
        height: 2,
        scene: {
          tileSize: 16,
          layers: [],
          concealedLayers: [],
          sprites: [],
          emitters: [emitter],
          concealedEmitters: [],
        },
      },
      textures: new Map(),
    } as unknown as MapBundle;
    try {
      const renderer = createMapParticleRenderer(
        context as unknown as CanvasRenderingContext2D,
        {} as HTMLCanvasElement,
        bundle,
        true,
      );
      renderer.draw(100);
      renderer.draw(100 + 1000 / 120);
      expect(context.translate.mock.calls[0][1]).not.toBe(context.translate.mock.calls[1][1]);
      expect(operations).toContain("lighter");
      expect(operations.at(-1)).toBe("destination-out");
      expect(
        createMapParticleRenderer(
          context as unknown as CanvasRenderingContext2D,
          {} as HTMLCanvasElement,
          bundle,
          false,
        ).animated,
      ).toBe(false);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("masks world fire behind walls while keeping status icons above walls", () => {
    const order: string[] = [];
    const maskContexts = [{ fillRect: vi.fn() }, { fillRect: vi.fn() }];
    let maskIndex = 0;
    vi.stubGlobal("document", {
      createElement: () => {
        const index = maskIndex++;
        return { label: index === 0 ? "darkness" : "wall", getContext: () => maskContexts[index] };
      },
    });
    const context = {
      canvas: { width: 16, height: 16 },
      save() {},
      restore() {},
      setTransform() {},
      clearRect() {},
      translate() {},
      rotate() {},
      scale() {},
      drawImage: (image: { label: string }) => order.push(image.label),
      fillRect: () => order.push("particle"),
    };
    const bundle = {
      textures: new Map(),
      map: {
        width: 1,
        height: 1,
        scene: {
          tileSize: 16,
          sprites: [
            {
              frames: [[{ kind: "fill", rgba: [255, 255, 255, 255], destination: [0, 12, 16, 4] }]],
            },
          ],
          layers: [{ name: "walls", cells: [0] }],
          concealedLayers: [],
          emitters: [emitter, { ...emitter, wallMask: false }],
          concealedEmitters: [],
        },
      },
    } as unknown as MapBundle;
    try {
      createMapParticleRenderer(
        context as unknown as CanvasRenderingContext2D,
        { label: "scenery" } as unknown as HTMLCanvasElement,
        bundle,
        true,
      ).draw(100);
      expect(maskContexts[1].fillRect).toHaveBeenCalledWith(0, 12, 16, 4);
      expect(order.lastIndexOf("wall")).toBeGreaterThan(order.indexOf("particle"));
      expect(order.lastIndexOf("wall")).toBeLessThan(order.lastIndexOf("particle"));
      expect(order.at(-1)).toBe("darkness");
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
