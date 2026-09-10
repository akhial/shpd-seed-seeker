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
  context.fillStyle = "#101216";
  context.fillRect(0, 0, map.width * tileSize, map.height * tileSize);
  context.imageSmoothingEnabled = false;
  for (const layer of map.scene.layers) {
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
  if (revealSecrets) {
    context.strokeStyle = "#ffd862";
    context.fillStyle = "#ffd86220";
    context.lineWidth = 1.5;
    for (const [left, top, right, bottom] of map.secretRooms) {
      const x = left * tileSize + 1,
        y = top * tileSize + 1;
      const w = (right - left + 1) * tileSize - 2,
        h = (bottom - top + 1) * tileSize - 2;
      context.fillRect(x, y, w, h);
      context.strokeRect(x, y, w, h);
    }
    for (const cell of [...map.secretDoors, ...map.secretTraps]) {
      context.strokeRect(
        (cell % map.width) * tileSize + 1,
        Math.floor(cell / map.width) * tileSize + 1,
        tileSize - 2,
        tileSize - 2,
      );
    }
  }
  // Tiny rings retain the original sprite while making navigation points legible at fit scale.
  const mark = (cell: number | null, color: string) => {
    if (cell === null) return;
    const x = ((cell % map.width) + 0.5) * tileSize,
      y = (Math.floor(cell / map.width) + 0.5) * tileSize;
    context.beginPath();
    context.arc(x, y, tileSize * 0.65, 0, 2 * Math.PI);
    context.strokeStyle = "#101216";
    context.lineWidth = 4;
    context.stroke();
    context.strokeStyle = color;
    context.lineWidth = 2;
    context.stroke();
  };
  mark(map.entrance, "#82daa0");
  mark(map.exit, "#9cd8ff");
  for (const branch of map.branches) mark(branch.entrance, "#d4b4fa");
}
