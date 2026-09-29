import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it } from "vite-plus/test";
import init, { scout } from "../../engine/pkg/seedfinder.js";
import type { ScoutResult } from "../../engine/types";
import { ScoutPanel } from "./ScoutPanel";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("../../engine/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});

function scoutWith(excludeSmithRewards: boolean): ScoutResult {
  return JSON.parse(
    scout(
      JSON.stringify({
        seed: "AAA-AAA-AAA",
        query: {
          auto_apply_trinket: false,
          exclude_blacksmith_rewards: excludeSmithRewards,
          requirements: [{ kind: "ring" }],
        },
      }),
    ),
  ) as ScoutResult;
}

/** The class of every item row the panel draws for a Blacksmith reward. */
function smithRowClasses(result: ScoutResult): string[] {
  const html = renderToStaticMarkup(
    <ScoutPanel
      input="AAA-AAA-AAA"
      onInput={() => {}}
      onScout={() => {}}
      loading={false}
      result={result}
    />,
  );
  return html
    .split('<li class="')
    .slice(1)
    .filter((row) => row.includes("Blacksmith Reward"))
    .map((row) => row.slice(0, row.indexOf('"')));
}

describe("scout item rows", () => {
  it("dims the Smith rewards a query excludes", () => {
    const excluded = scoutWith(true);
    const rewards = excluded.items.filter((item) => item.source === "blacksmith_reward");
    expect(rewards.length).toBeGreaterThan(0);
    for (const item of excluded.items) {
      expect(item.excluded).toBe(item.source === "blacksmith_reward");
      if (item.excluded) expect(item.matched).toBe(false);
    }
    const dimmed = smithRowClasses(excluded);
    expect(dimmed).toHaveLength(rewards.length);
    for (const row of dimmed) expect(row).toBe("d1-item d1-item-dimmed");

    // With the option off the same rewards are ordinary, undimmed rows.
    const allowed = scoutWith(false);
    expect(allowed.items.some((item) => item.excluded)).toBe(false);
    const plain = smithRowClasses(allowed);
    expect(plain).toHaveLength(rewards.length);
    for (const row of plain) expect(row).toBe("d1-item");
  });
});
