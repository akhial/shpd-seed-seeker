export type Rectangle = [number, number, number, number];
export type MapDraw =
  | { kind: "blit"; asset: string; source: Rectangle; destination: Rectangle }
  | { kind: "fill"; rgba: [number, number, number, number]; destination: Rectangle };
export interface MapSprite {
  frameDurationMs: number;
  frames: MapDraw[][];
}
export interface LevelMapDocument {
  format: "seed-seeker-level-map";
  schemaVersion: 2;
  seed: string;
  depth: number;
  branch: number;
  kind: "regular" | "blacksmith_crystal" | "blacksmith_gnoll" | "imp_vault";
  challenges: string[];
  selectedTrinket: string | null;
  width: number;
  height: number;
  terrain: number[];
  entrance: number | null;
  exit: number | null;
  secretRooms: Rectangle[];
  secretDoors: number[];
  secretTraps: number[];
  traps: { cell: number; kind: string; hidden: boolean; active: boolean }[];
  branches: { depth: number; branch: number; kind: string; entrance: number }[];
  assets: { id: string; width: number; height: number; sha256: string }[];
  scene: {
    tileSize: number;
    sprites: MapSprite[];
    layers: { name: string; cells: (number | null)[] }[];
    concealedLayers: { name: string; cells: (number | null)[] }[];
  };
}
export interface LevelMapRequest {
  seed: string;
  depth: number;
  branch?: number;
  challenges: readonly string[];
  selectedTrinket?: string | null;
}
export interface MapBundle {
  map: LevelMapDocument;
  textures: ReadonlyMap<string, ImageBitmap>;
}
export interface MapWorkerRequest {
  id: number;
  requestJson: string;
}
export type MapWorkerResponse =
  | { id: number; map: LevelMapDocument; assets: { id: string; png: ArrayBuffer }[] }
  | { id: number; error: string };
