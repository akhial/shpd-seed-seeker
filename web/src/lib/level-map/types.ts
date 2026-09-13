import type { ChallengeName } from "../wasm/types";

export type Rectangle = [number, number, number, number];
export interface MapGlow {
  color: [number, number, number];
  periodMs: number;
}
export type MapDraw =
  | {
      kind: "blit";
      asset: string;
      source: Rectangle;
      opacity?: number;
      tint?: [number, number, number];
      glow?: MapGlow;
      destination: Rectangle;
    }
  | { kind: "fill"; rgba: [number, number, number, number]; destination: Rectangle };
export interface MapSprite {
  frameDurationMs: number;
  frames: MapDraw[][];
}
/** Objects present after generation; runtime-dependent identities are explicit. */
export interface MapItem {
  kind: string;
  image: number;
  quantity: number;
  deterministic: boolean;
  glow?: MapGlow;
}
export interface MapContents {
  heaps: { cell: number; kind: string; haunted: boolean; items: MapItem[] }[];
  mobs: {
    cell: number;
    kind: string;
    stealthy: boolean;
    sleeping: boolean;
    approximate: boolean;
    items: MapItem[];
  }[];
  plants: { cell: number; kind: string; image: number }[];
  effects: { cell: number; kind: string }[];
  features: { cell: number; kind: string; width: number; height: number }[];
  traps: { cell: number; kind: string; hidden: boolean; active: boolean }[];
}
export interface LevelMapDocument {
  format: "seed-seeker-level-map";
  schemaVersion: 2 | 3;
  seed: string;
  depth: number;
  branch: number;
  kind: "regular" | "blacksmith_crystal" | "blacksmith_gnoll" | "imp_vault";
  challenges: ChallengeName[];
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
  /** Optional for older v2 engines; drawing is always defined by scene. */
  contents?: MapContents;
  pickupAssumptions?: { earlierHourglass: "take_identify_uncurse"; shopSand: "buy" };
  scene: {
    emitters?: MapEmitter[];
    concealedEmitters?: MapEmitter[];
    tileSize: number;
    sprites: MapSprite[];
    layers: { name: string; blend?: "add"; cells: (number | null)[] }[];
    concealedLayers: { name: string; blend?: "add"; cells: (number | null)[] }[];
  };
}
export interface LevelMapRequest {
  seed: string;
  depth: number;
  branch?: number;
  challenges: readonly ChallengeName[];
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

export interface MapCurve {
  points: [number, number][];
  sqrt: boolean;
}
export interface MapEmitter {
  wallMask?: boolean;
  cell: number;
  loopMs: number;
  blend: "add" | null;
  image: MapDraw;
  velocity: [number, number];
  acceleration: [number, number];
  angularSpeed: number;
  alpha: MapCurve;
  scale: MapCurve;
  particles: {
    birthMs: number;
    lifespanMs: number;
    position: [number, number];
    scale: number;
    angle: number;
  }[];
}
