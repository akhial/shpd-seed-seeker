import { Store } from "@tanstack/store";
import {
  defaultQueryState,
  defaultTier,
  defaultUpgrade,
  fromQueryJson,
  toQueryJson,
} from "../features/query/query";
import type { QueryState } from "../engine/types";

const QUERY_KEY = "seedseeker.query.v1";
const PRESETS_KEY = "seedseeker.presets.v1";
const WORKERS_KEY = "seedseeker.workers.v1";

/** Logical processors available for search workers, always at least 1. */
export const maxWorkers = (): number =>
  Math.max(1, (typeof navigator !== "undefined" && navigator.hardwareConcurrency) || 4);

function hydrateWorkerCount(): number {
  const ceiling = maxWorkers();
  if (typeof localStorage === "undefined") return ceiling;
  const saved = Number(localStorage.getItem(WORKERS_KEY));
  if (!Number.isFinite(saved) || saved < 1) return ceiling;
  return Math.min(Math.floor(saved), ceiling);
}

export const workerCountStore = new Store<number>(hydrateWorkerCount());
export function setWorkerCount(count: number): void {
  const clamped = Math.min(Math.max(1, Math.floor(count)), maxWorkers());
  workerCountStore.setState(() => clamped);
  if (typeof localStorage !== "undefined") localStorage.setItem(WORKERS_KEY, String(clamped));
}

function hydrateQuery(): QueryState {
  if (typeof localStorage === "undefined") return defaultQueryState();
  try {
    const saved = localStorage.getItem(QUERY_KEY);
    return saved ? fromQueryJson(saved) : defaultQueryState();
  } catch {
    return defaultQueryState();
  }
}

export const queryStore = new Store<QueryState>(hydrateQuery());
if (typeof localStorage !== "undefined") {
  queryStore.subscribe(() => localStorage.setItem(QUERY_KEY, toQueryJson(queryStore.state)));
}

export interface Preset {
  name: string;
  query: QueryState;
}

export const builtInPresets: Preset[] = [
  {
    name: "DISINTEGRATE",
    query: {
      ...defaultQueryState(),
      maxDepth: 19,
      requirements: [
        {
          key: 1,
          kind: "wand",
          item: "wand_disintegration",
          tier: defaultTier(),
          upgrade: { mode: "at_least", value: 3 },
          uncursed: false,
        },
        {
          key: 2,
          kind: "wand",
          item: "wand_disintegration",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
        },
        {
          key: 3,
          kind: "wand",
          item: "wand_disintegration",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
        },
        {
          key: 4,
          kind: "trinket",
          item: "eye_of_newt",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
          trinketTransmutations: 1,
        },
        {
          key: 5,
          kind: "ring",
          item: "ring_energy",
          tier: defaultTier(),
          upgrade: { mode: "at_least", value: 2 },
          uncursed: false,
        },
      ],
    },
  },
  {
    name: "Guerilla Assassin",
    query: {
      ...defaultQueryState(),
      requirements: [
        {
          key: 1,
          kind: "weapon",
          item: "assassins_blade",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 3 },
          effect: "Blooming",
          uncursed: false,
          maxDepth: 7,
        },
        {
          key: 2,
          kind: "armor",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          effect: "Camouflage",
          uncursed: false,
        },
        {
          key: 3,
          kind: "ring",
          item: "ring_arcana",
          tier: defaultTier(),
          upgrade: { mode: "at_least", value: 2 },
          uncursed: false,
        },
      ],
    },
  },
  {
    // Early gear for a wealth run, and the dark garden floor 17 farms on.
    name: "Ring of Wealth",
    query: {
      ...defaultQueryState(),
      floorRequirements: [{ depth: 17, feeling: "dark", any_rooms: ["garden", "secret_garden"] }],
      requirements: [
        {
          key: 1,
          kind: "ring",
          item: "ring_wealth",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 4 },
          uncursed: false,
        },
        {
          key: 2,
          kind: "artifact",
          item: "dried_rose",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
          maxDepth: 9,
        },
        {
          key: 3,
          kind: "armor",
          tier: { mode: "at_most", value: 4 },
          upgrade: { mode: "exact", value: 3 },
          uncursed: false,
          maxDepth: 4,
        },
        {
          key: 4,
          kind: "weapon",
          tier: { mode: "at_most", value: 4 },
          upgrade: { mode: "exact", value: 3 },
          uncursed: false,
          maxDepth: 9,
        },
        {
          key: 5,
          kind: "trinket",
          item: "dimensional_sundial",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
          trinketTransmutations: 1,
        },
      ],
    },
  },
  {
    name: "Necromancer",
    query: {
      ...defaultQueryState(),
      maxDepth: 14,
      wandmakerQuest: "corpse_dust",
      requirements: [
        {
          key: 1,
          kind: "wand",
          item: "wand_corruption",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 3 },
          uncursed: false,
        },
        {
          key: 2,
          kind: "weapon",
          tier: { mode: "exact", value: 5 },
          upgrade: { mode: "exact", value: 3 },
          uncursed: false,
        },
        {
          key: 3,
          kind: "armor",
          item: "plate_armor",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 3 },
          uncursed: false,
        },
      ],
    },
  },
  {
    name: "Blood Berserker",
    query: {
      ...defaultQueryState(),
      requirements: [
        {
          key: 1,
          kind: "weapon",
          tier: { mode: "exact", value: 5 },
          upgrade: { mode: "exact", value: 3 },
          effect: "Vampiric",
          uncursed: false,
        },
        {
          key: 2,
          kind: "armor",
          item: "plate_armor",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 3 },
          effect: "Thorns",
          uncursed: false,
        },
        {
          key: 3,
          kind: "ring",
          item: "ring_arcana",
          tier: defaultTier(),
          upgrade: { mode: "exact", value: 4 },
          uncursed: false,
        },
        {
          key: 4,
          kind: "artifact",
          item: "chalice_of_blood",
          tier: defaultTier(),
          upgrade: defaultUpgrade(),
          uncursed: false,
        },
      ],
    },
  },
];

export function loadPresets(): Preset[] {
  try {
    const value = localStorage.getItem(PRESETS_KEY);
    if (!value) return [];
    const raw = JSON.parse(value) as { name: string; query: unknown }[];
    return raw.map((preset) => ({
      name: preset.name,
      query: fromQueryJson(JSON.stringify(preset.query)),
    }));
  } catch {
    return [];
  }
}

export function savePresets(presets: Preset[]): void {
  localStorage.setItem(
    PRESETS_KEY,
    JSON.stringify(
      presets.map((preset) => ({
        name: preset.name,
        query: JSON.parse(toQueryJson(preset.query)),
      })),
    ),
  );
}
