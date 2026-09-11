import type { MapBundle, MapSprite } from "./types";

export function spriteFrame(sprite: MapSprite, elapsed: number) {
  return sprite.frames[
    Math.floor(Math.max(0, elapsed) / Math.max(1, sprite.frameDurationMs)) % sprite.frames.length
  ];
}
export function drawLevelMap(
  context: CanvasRenderingContext2D,
  { map, textures }: MapBundle,
  elapsed: number,
  revealSecrets: boolean,
) {
  const tileSize = map.scene.tileSize;
  context.clearRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.fillStyle = "#000";
  context.fillRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.imageSmoothingEnabled = false;
  for (const layer of revealSecrets ? map.scene.layers : map.scene.concealedLayers) {
    for (let cell = 0; cell < layer.cells.length; cell++) {
      const index = layer.cells[cell];
      if (index === null) continue;
      const ox = (cell % map.width) * tileSize;
      const oy = Math.floor(cell / map.width) * tileSize;
      for (const draw of spriteFrame(map.scene.sprites[index], elapsed)) {
        const [x, y, w, h] = draw.destination;
        if (draw.kind === "blit") {
          context.drawImage(textures.get(draw.asset)!, ...draw.source, ox + x, oy + y, w, h);
        } else {
          const [r, g, b, a] = draw.rgba;
          context.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
          context.fillRect(ox + x, oy + y, w, h);
        }
      }
    }
  }
}
