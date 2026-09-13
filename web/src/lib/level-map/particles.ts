import { drawTexture } from "./textures";
import { drawCommand } from "./render";
import type { MapBundle, MapCurve, MapEmitter, Rectangle } from "./types";

export function curveValue(curve: MapCurve, progress: number): number {
  const points = curve.points;
  const p = Math.max(0, Math.min(1, progress)) * 1000;
  const right = points.findIndex(([x]) => x >= p);
  let value: number;
  if (right <= 0) value = points[right === 0 ? 0 : points.length - 1][1];
  else {
    const [x0, y0] = points[right - 1],
      [x1, y1] = points[right];
    value = y0 + ((y1 - y0) * (p - x0)) / (x1 - x0);
  }
  value /= 1000;
  return curve.sqrt ? Math.sqrt(value) : value;
}

export function particleState(
  emitter: MapEmitter,
  particle: MapEmitter["particles"][number],
  elapsed: number,
) {
  const clock = Math.max(0, elapsed) - (emitter.startMs ?? 0);
  // Scheduled hazards start with the captured first turn; ambient emitters
  // retain their prewarmed loops. Never wrap a future first emission backward.
  if (emitter.startMs !== undefined && clock < particle.birthMs) return null;
  const age = (((clock - particle.birthMs) % emitter.loopMs) + emitter.loopMs) % emitter.loopMs;
  if (age >= particle.lifespanMs) return null;
  const seconds = age / 1000,
    progress = age / particle.lifespanMs;
  return {
    x:
      particle.position[0] / 1000 +
      emitter.velocity[0] * seconds +
      (emitter.acceleration[0] * seconds * seconds) / 2,
    y:
      particle.position[1] / 1000 +
      emitter.velocity[1] * seconds +
      (emitter.acceleration[1] * seconds * seconds) / 2,
    scale: (particle.scale / 1000) * curveValue(emitter.scale, progress),
    alpha: curveValue(emitter.alpha, progress),
    scaleY: emitter.scaleY ? curveValue(emitter.scaleY, progress) : 1,
    angle: ((particle.angle + emitter.angularSpeed * seconds) * Math.PI) / 180,
  };
}

function emitterBounds(emitter: MapEmitter, width: number, size: number): Rectangle {
  const positions: [number, number][] = [];
  for (const particle of emitter.particles) {
    // Include trajectory extrema, not just its endpoints (e.g. upward sparks).
    const times = [0, particle.lifespanMs / 1000];
    for (let axis = 0; axis < 2; axis++) {
      const t = -emitter.velocity[axis] / emitter.acceleration[axis];
      if (t > 0 && t < times[1]) times.push(t);
    }
    for (const t of times)
      positions.push(
        [0, 1].map(
          (axis) =>
            particle.position[axis] / 1000 +
            emitter.velocity[axis] * t +
            (emitter.acceleration[axis] * t * t) / 2,
        ) as [number, number],
      );
  }
  const maxCurve = Math.max(...emitter.scale.points.map(([, value]) => value)) / 1000;
  const maxScale =
    ((emitter.scale.sqrt ? Math.sqrt(maxCurve) : maxCurve) *
      Math.max(...emitter.particles.map((particle) => particle.scale))) /
    1000;
  const radius =
    (Math.hypot(
      emitter.image.destination[2],
      emitter.image.destination[3] *
        (emitter.scaleY
          ? Math.max(...emitter.scaleY.points.map(([x]) => curveValue(emitter.scaleY!, x / 1000)))
          : 1),
    ) *
      maxScale) /
      2 +
    1;
  const x = Math.floor(
    (emitter.cell % width) * size + Math.min(...positions.map(([x]) => x)) - radius,
  );
  const y = Math.floor(
    Math.floor(emitter.cell / width) * size + Math.min(...positions.map(([, y]) => y)) - radius,
  );
  return [
    x,
    y,
    Math.ceil(Math.max(...positions.map(([x]) => x))) -
      Math.floor(Math.min(...positions.map(([x]) => x))) +
      Math.ceil(radius) * 2 +
      2,
    Math.ceil(Math.max(...positions.map(([, y]) => y))) -
      Math.floor(Math.min(...positions.map(([, y]) => y))) +
      Math.ceil(radius) * 2 +
      2,
  ];
}

/** Separate display-rate surface. Only particle bounds are copied from scenery,
 * providing the destination pixels required by the game's additive blending. */
export function createMapParticleRenderer(
  context: CanvasRenderingContext2D,
  scenery: HTMLCanvasElement,
  bundle: MapBundle,
  revealSecrets: boolean,
) {
  const { map } = bundle,
    size = map.scene.tileSize;
  const width = map.width * size,
    height = map.height * size;
  const emitters = (revealSecrets ? map.scene.emitters : map.scene.concealedEmitters) ?? [];
  const regions = emitters
    .map((emitter) => emitterBounds(emitter, map.width, size))
    .map(([x, y, w, h]): Rectangle => {
      const left = Math.max(0, x),
        top = Math.max(0, y);
      return [
        left,
        top,
        Math.max(0, Math.min(width, x + w) - left),
        Math.max(0, Math.min(height, y + h) - top),
      ];
    });
  const layers = revealSecrets ? map.scene.layers : map.scene.concealedLayers;
  const makeMask = (names: string[]) => {
    const mask = document.createElement("canvas");
    mask.width = width;
    mask.height = height;
    const target = mask.getContext("2d")!;
    target.imageSmoothingEnabled = false;
    for (const layer of layers.filter((layer) => names.includes(layer.name))) {
      layer.cells.forEach((sprite, cell) => {
        if (sprite === null) return;
        for (const draw of map.scene.sprites[sprite].frames[0])
          drawCommand(
            target,
            bundle,
            draw,
            (cell % map.width) * size,
            Math.floor(cell / map.width) * size,
          );
      });
    }
    return mask;
  };
  const darkness = makeMask(["darkness"]);
  const walls = makeMask(["raised", "walls", "room_walls", "boss_walls"]);
  // Resolve tinted atlas pixels once, outside the display-rate loop.
  const images = emitters.map((emitter) =>
    emitter.image.kind === "blit" ? drawTexture(bundle, emitter.image) : null,
  );
  return {
    animated: emitters.length > 0,
    draw(elapsed: number) {
      context.save();
      context.setTransform(
        context.canvas.width / width,
        0,
        0,
        context.canvas.height / height,
        0,
        0,
      );
      context.imageSmoothingEnabled = false;
      context.globalAlpha = 1;
      context.globalCompositeOperation = "source-over";
      // Copy all backgrounds before drawing any emitter; overlaps retain particles.
      for (const [x, y, w, h] of regions) {
        context.clearRect(x, y, w, h);
        if (w && h) context.drawImage(scenery, x, y, w, h, x, y, w, h);
      }
      const drawEmitter = (emitter: MapEmitter, index: number) => {
        const ox = (emitter.cell % map.width) * size,
          oy = Math.floor(emitter.cell / map.width) * size;
        const image = emitter.image,
          w = image.destination[2],
          h = image.destination[3];
        context.globalCompositeOperation = emitter.blend === "add" ? "lighter" : "source-over";
        for (const particle of emitter.particles) {
          const state = particleState(emitter, particle, elapsed);
          if (!state || state.scale <= 0 || state.alpha <= 0) continue;
          context.save();
          context.translate(ox + state.x, oy + state.y);
          if (state.angle) context.rotate(state.angle);
          context.scale(state.scale, state.scale * state.scaleY);
          context.globalAlpha =
            state.alpha *
            (image.kind === "blit" ? (image.opacity ?? 255) / 255 : image.rgba[3] / 255);
          if (image.kind === "blit")
            context.drawImage(images[index]!, ...image.source, -w / 2, -h / 2, w, h);
          else {
            context.fillStyle = `rgb(${image.rgba.slice(0, 3).join(",")})`;
            context.fillRect(-w / 2, -h / 2, w, h);
          }
          context.restore();
        }
      };
      const eraseMask = (mask: HTMLCanvasElement) => {
        context.globalAlpha = 1;
        context.globalCompositeOperation = "destination-out";
        for (const [x, y, w, h] of regions)
          if (w && h) context.drawImage(mask, x, y, w, h, x, y, w, h);
      };
      emitters.forEach((emitter, index) => {
        if (emitter.wallMask) drawEmitter(emitter, index);
      });
      eraseMask(walls);
      emitters.forEach((emitter, index) => {
        if (!emitter.wallMask) drawEmitter(emitter, index);
      });
      eraseMask(darkness);
      context.restore();
    },
  };
}
