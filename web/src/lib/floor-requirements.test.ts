import { readFile } from "node:fs/promises";
import { beforeAll, expect, it } from "vite-plus/test";
import {
  FARMING_FLOORS,
  ROOM_TYPES,
  isFarmingRequirement,
  toggleFarmingFloor,
} from "./floor-requirements";
import {
  defaultQueryState,
  fromQueryJson,
  toQueryDocument,
  toQueryJson,
  validateQuery,
} from "./query";
import { decodeResultsFile, encodeResultsFile } from "./results-file";
import init, {
  analyze_query,
  decode_share_text,
  encode_share_link,
  engine_info,
  scout,
  filter_seeds,
} from "./wasm/pkg/seedfinder.js";
import type { EngineInfo, ScoutResult } from "./wasm/types";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("./wasm/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});
it("keeps the portable room catalog in sync with the engine", () => {
  expect(ROOM_TYPES).toEqual((JSON.parse(engine_info()) as EngineInfo).roomTypes);
});
it("selects each farming floor independently and raises the scope only as needed", () => {
  let query = { ...defaultQueryState(), maxDepth: 4 };
  for (const depth of FARMING_FLOORS) query = toggleFarmingFloor(query, depth);
  expect(query.maxDepth).toBe(22);
  expect(query.floorRequirements?.map((floor) => floor.depth)).toEqual([7, 17, 22]);
  expect(query.floorRequirements?.every(isFarmingRequirement)).toBe(true);
  expect(query.requirements).toEqual([]);
  expect(validateQuery(query).valid).toBe(true);
  query = toggleFarmingFloor(query, 17);
  expect(query.floorRequirements?.map((floor) => floor.depth)).toEqual([7, 22]);
  expect(query.maxDepth).toBe(22);
  expect(validateQuery({ ...query, maxDepth: 16 }).errors).toContain(
    "Floor 22 exceeds the floor limit of 16.",
  );
});
it("preserves farming floors through query state, links, and results files", () => {
  let query = defaultQueryState();
  for (const depth of FARMING_FLOORS) query = toggleFarmingFloor(query, depth);
  expect(fromQueryJson(toQueryJson(query))).toEqual(query);
  expect(fromQueryJson(decode_share_text(encode_share_link(toQueryJson(query))))).toEqual(query);
  expect(decodeResultsFile(encodeResultsFile(toQueryDocument(query), [])).query).toEqual(query);
  const analysis = JSON.parse(analyze_query(toQueryJson(query)));
  expect(analysis.valid).toBe(true);
  expect(analysis.impossible).toBe(false);
  expect(analysis.probability).toBeGreaterThan(0);
  expect(analysis.probability).toBeLessThan(0.001);
});
it("retains general floor filters and rejects malformed input", () => {
  const query = fromQueryJson(
    JSON.stringify({
      requirements: [],
      floor_requirements: [
        {
          depth: 9,
          feeling: "secrets",
          rooms: ["secret_library"],
          any_rooms: ["garden", "secret_garden"],
        },
      ],
    }),
  );
  expect(fromQueryJson(decode_share_text(encode_share_link(toQueryJson(query))))).toEqual(query);
  for (const filter of [
    { depth: 5, feeling: "dark" },
    { depth: 7, feeling: "typo" },
    { depth: 7, rooms: ["typo"] },
    { depth: 7, rooms: null },
    { depth: 7 },
  ]) {
    expect(() =>
      fromQueryJson(JSON.stringify({ requirements: [], floor_requirements: [filter] })),
    ).toThrow();
  }
});
it("scouts room summaries and checks floor-only conditions when filtering saved seeds", () => {
  const world = JSON.parse(scout(JSON.stringify({ seed: "AAA-AAA-AAA" }))) as ScoutResult;
  expect(world.floorRooms?.length).toBe(20);
  const floor = world.floorRooms!.find((floor) => floor.depth === 17)!;
  const feeling = world.feelings!.find((floor) => floor.depth === 17)!.feeling;
  const query = JSON.stringify({
    requirements: [],
    floor_requirements: [{ depth: 17, feeling, rooms: floor.rooms }],
  });
  expect(JSON.parse(filter_seeds(query, new Float64Array([world.seed.value])))).toHaveLength(1);
  const rejected = JSON.stringify({
    requirements: [],
    floor_requirements: [{ depth: 17, feeling: feeling === "dark" ? "water" : "dark" }],
  });
  expect(JSON.parse(filter_seeds(rejected, new Float64Array([world.seed.value])))).toHaveLength(0);
});
