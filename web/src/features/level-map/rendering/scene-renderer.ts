import type { MapContext } from "./canvas";
import { createLevelMapRenderer } from "./render";
import { createMapParticleRenderer } from "./particles";
import type { MapBundle } from "../types";

/** Preserve the original two surfaces and particle density in either thread. */
export function createMapSceneRenderer(
  scenery: MapContext,
  effects: MapContext,
  bundle: MapBundle,
  revealSecrets: boolean,
) {
  const renderer = createLevelMapRenderer(scenery, bundle, revealSecrets);
  const particles = createMapParticleRenderer(effects, scenery.canvas, bundle, revealSecrets);
  return {
    animated: renderer.animated || particles.animated,
    draw(elapsed: number, advanceSprites = true, density = 1) {
      renderer.draw(elapsed, advanceSprites);
      const width = Math.round(bundle.map.width * bundle.map.scene.tileSize * density);
      const height = Math.round(bundle.map.height * bundle.map.scene.tileSize * density);
      if (effects.canvas.width !== width || effects.canvas.height !== height) {
        effects.canvas.width = width;
        effects.canvas.height = height;
      }
      particles.draw(elapsed);
    },
  };
}
