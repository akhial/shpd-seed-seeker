import { readFile } from "node:fs/promises";
import { beforeAll, describe, expect, it } from "vite-plus/test";
import { LEVEL_GEN_CHALLENGES, challenges } from "../shared/game/catalog";
import {
  BLACKSMITH_LAST_FLOOR,
  EMPTY_BOSS_FLOORS,
  MAX_DEPTH,
  TRINKET_TRANSMUTATION_MAX,
} from "../features/query/query";
import { RESULT_CAP } from "../features/search/coordinator-state";
import { SEARCH_START_STRIDE, TOTAL_SEEDS } from "../features/search/traversal";
import init, { engine_info } from "./pkg/seedfinder.js";
import type { EngineInfo } from "./types";

/**
 * The app keeps local copies of the engine's scalar constants so nothing has
 * to wait on the wasm module to render. This is the one place they meet the
 * engine: every local is asserted against the `engine_info` document, so a
 * change on either side fails here rather than as an editor offering a query
 * the search refuses. Node has no `fetch` for `file:` URLs, so the module is
 * instantiated from bytes.
 */
let info: EngineInfo;

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("./pkg/seedfinder_bg.wasm", import.meta.url)),
  });
  info = JSON.parse(engine_info()) as EngineInfo;
});

describe("local constants match the engine document", () => {
  it("query bounds", () => {
    // The requirement sheet's bounds (tiers, upgrades, stacks, group labels)
    // are the shared core's, drawn in its form; the app keeps no copy.
    expect(MAX_DEPTH).toBe(info.limits.maxDepth);
    expect(TRINKET_TRANSMUTATION_MAX).toBe(info.limits.trinketTransmutationsMax);
  });

  it("result cap and seed space", () => {
    expect(RESULT_CAP).toBe(info.maxResults);
    expect(TOTAL_SEEDS).toBe(info.totalSeeds);
    expect(SEARCH_START_STRIDE).toBe(info.searchStartStride);
    // The import byte cap is applied by the engine's own decoder; the app
    // keeps no copy of it, so there is nothing local to compare.
    expect(info.limits.resultsFileMaxBytes).toBeGreaterThan(0);
  });

  it("empty boss floors and the Blacksmith window", () => {
    expect([...EMPTY_BOSS_FLOORS]).toEqual(info.emptyBossFloors);
    // The app has no quest-window table of its own; the only window it
    // depends on is the Blacksmith's last floor, which gates "require
    // Blacksmith".
    expect(BLACKSMITH_LAST_FLOOR).toBe(info.questWindows.blacksmith[1]);
  });

  it("challenge list, mask order, and generation relevance", () => {
    expect(challenges.map((challenge) => challenge.value)).toEqual(
      info.challenges.map((challenge) => challenge.name),
    );
    info.challenges.forEach((challenge, index) => expect(challenge.mask).toBe(1 << index));
    expect([...LEVEL_GEN_CHALLENGES].sort()).toEqual(
      info.challenges
        .filter((challenge) => challenge.changesLevelGeneration)
        .map((challenge) => challenge.name)
        .sort(),
    );
  });
});
