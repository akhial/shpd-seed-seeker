import type { FloorFeeling, FloorRequirement, QueryState } from "../../engine/types";

// Stable engine IDs, checked against engine_info in floor-requirements.test.ts.
export const ROOM_TYPES = [
  "standard_sewer_pipe",
  "standard_ring",
  "standard_water_bridge",
  "standard_region_deco_patch",
  "standard_circle_basin",
  "standard_region_deco_line",
  "standard_segmented",
  "standard_pillars",
  "standard_chasm_bridge",
  "standard_cell_block",
  "standard_cave",
  "standard_region_deco_bridge",
  "standard_caves_fissure",
  "standard_circle_pit",
  "standard_circle_wall",
  "standard_hallway",
  "standard_library_hall",
  "standard_library_ring",
  "standard_statues",
  "standard_segmented_library",
  "standard_ruins",
  "standard_chasm",
  "standard_skulls",
  "standard_ritual",
  "standard_plants",
  "standard_aquarium",
  "standard_platform",
  "standard_burned",
  "standard_fissure",
  "standard_grassy_grave",
  "standard_striped",
  "standard_study",
  "standard_suspicious_chest",
  "standard_minefield",
  "standard_mine_entrance",
  "standard_mine_small",
  "standard_mine_large",
  "standard_mine_giant",
  "standard_sewer_boss_entrance",
  "standard_sewer_boss_exit",
  "standard_diamond_goo",
  "standard_walled_goo",
  "standard_thin_pillars_goo",
  "standard_thick_pillars_goo",
  "connection_tunnel",
  "connection_bridge",
  "connection_perimeter",
  "connection_walkway",
  "connection_ring_tunnel",
  "connection_ring_bridge",
  "connection_maze",
  "weak_floor",
  "crypt",
  "pool",
  "armory",
  "sentry",
  "statue",
  "crystal_vault",
  "crystal_choice",
  "sacrifice",
  "runestone",
  "garden",
  "library",
  "storage",
  "treasury",
  "magic_well",
  "toxic_gas",
  "magical_fire",
  "traps",
  "crystal_path",
  "laboratory",
  "pit",
  "shop",
  "demon_spawner",
  "secret_garden",
  "secret_laboratory",
  "secret_library",
  "secret_larder",
  "secret_well",
  "secret_runestone",
  "secret_artillery",
  "secret_chest_chasm",
  "secret_honeypot",
  "secret_hoard",
  "secret_maze",
  "secret_summoning",
  "secret_mine",
  "secret_rat_king",
  "quest_mass_grave",
  "quest_ritual_site",
  "quest_rot_garden",
  "quest_blacksmith",
  "quest_ambitious_imp",
  "entrance",
  "exit",
] as const;
export type RoomType = (typeof ROOM_TYPES)[number];
export const FARMING_FLOORS = [7, 17, 22] as const;
export const FEELINGS: readonly FloorFeeling[] = [
  "none",
  "chasm",
  "water",
  "grass",
  "dark",
  "large",
  "traps",
  "secrets",
];

export function isFarmingRequirement(floor: FloorRequirement): boolean {
  return (
    FARMING_FLOORS.some((depth) => depth === floor.depth) &&
    floor.feeling === "dark" &&
    !floor.rooms?.length &&
    floor.any_rooms?.length === 2 &&
    floor.any_rooms.includes("garden") &&
    floor.any_rooms.includes("secret_garden")
  );
}

export function toggleFarmingFloor(query: QueryState, depth: number): QueryState {
  const floors = query.floorRequirements ?? [];
  const selected = floors.some((floor) => floor.depth === depth && isFarmingRequirement(floor));
  return {
    ...query,
    maxDepth: selected ? query.maxDepth : Math.max(query.maxDepth, depth),
    floorRequirements: [
      ...floors.filter((floor) => floor.depth !== depth),
      ...(selected
        ? []
        : [
            {
              depth,
              feeling: "dark" as const,
              any_rooms: ["garden", "secret_garden"] as RoomType[],
            },
          ]),
    ].sort((a, b) => a.depth - b.depth),
  };
}

export function floorRequirementErrors(
  floors: readonly FloorRequirement[],
  maxDepth: number,
): string[] {
  const errors: string[] = [];
  const seen = new Set<number>();
  for (const floor of floors) {
    if (
      !Number.isInteger(floor.depth) ||
      floor.depth < 1 ||
      floor.depth > 24 ||
      floor.depth % 5 === 0
    )
      errors.push("Choose a regular floor from 1 through 24.");
    if (floor.depth > maxDepth)
      errors.push(`Floor ${floor.depth} exceeds the floor limit of ${maxDepth}.`);
    if (seen.has(floor.depth)) errors.push(`Floor ${floor.depth} has more than one requirement.`);
    seen.add(floor.depth);
    if (floor.feeling !== undefined && !FEELINGS.includes(floor.feeling))
      errors.push("Unknown floor feeling.");
    if (floor.feeling === undefined && !floor.rooms?.length && !floor.any_rooms?.length)
      errors.push("Choose a feeling or a room for each floor.");
    for (const rooms of [floor.rooms, floor.any_rooms]) {
      if (
        rooms !== undefined &&
        (!Array.isArray(rooms) || rooms.some((room) => !ROOM_TYPES.includes(room)))
      )
        errors.push("Unknown room type.");
    }
  }
  return errors;
}

export function floorsFromDocument(value: unknown): FloorRequirement[] | undefined {
  if (value === undefined) return undefined;
  if (
    !Array.isArray(value) ||
    value.some(
      (floor) =>
        !floor ||
        typeof floor !== "object" ||
        Object.keys(floor).some((key) => !["depth", "feeling", "rooms", "any_rooms"].includes(key)),
    )
  )
    throw new Error("Invalid floor requirements.");
  const floors = value as FloorRequirement[];
  const errors = floorRequirementErrors(floors, 24);
  if (errors.length) throw new Error(errors[0]);
  return floors;
}
