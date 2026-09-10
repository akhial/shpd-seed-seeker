import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vite-plus/test";
import init from "../../lib/wasm/pkg/seedfinder.js";
import type { ScoutResult } from "../../lib/wasm/types";
import { FloorMapHeader } from "./FloorMapHeader";
import { ScoutPanel } from "./ScoutPanel";

vi.mock("./LevelMapView", () => ({
  LevelMapView: () => {
    throw new Error("A collapsed floor must not mount a map renderer");
  },
}));

beforeAll(async () => {
  await init({
    module_or_path: await readFile(
      new URL("../../lib/wasm/pkg/seedfinder_bg.wasm", import.meta.url),
    ),
  });
});

describe("inline scout maps", () => {
  it("offers a keyboard-accessible map disclosure alongside the floor and quest context", () => {
    const html = renderToStaticMarkup(
      <FloorMapHeader
        depth={13}
        feeling="water"
        quest={{ quest: "blacksmith", variant: "crystal", depth: 13 }}
        expanded={false}
        onToggle={() => {}}
        onPrefetch={() => {}}
      />,
    );
    expect(html).toContain('<button type="button"');
    expect(html).toContain('aria-expanded="false"');
    expect(html).toContain('aria-controls="scout-floor-map-13"');
    expect(html).toContain('aria-label="Show floor 13 map"');
    expect(html).toContain("Floor 13");
    expect(html).toContain("Caves");
    expect(html).toContain("Crystal");
    expect(html).toContain(">Map<");
  });

  it("reports expanded state and never offers unsupported boss maps", () => {
    const opened = renderToStaticMarkup(
      <FloorMapHeader depth={18} expanded onToggle={() => {}} onPrefetch={() => {}} />,
    );
    expect(opened).toContain('aria-expanded="true"');
    expect(opened).toContain('aria-label="Hide floor 18 map"');
    for (const depth of [5, 10, 15, 20, 25]) {
      const html = renderToStaticMarkup(
        <FloorMapHeader depth={depth} expanded={false} onToggle={() => {}} onPrefetch={() => {}} />,
      );
      expect(html).toContain(`Floor ${depth}`);
      expect(html).not.toContain("<button");
      expect(html).not.toContain(">Map<");
    }
  });

  it("keeps maps discoverable for generated floors with no loot and loads none initially", () => {
    const result: ScoutResult = {
      seed: { code: "AAA-AAA-AAA", value: 0 },
      items: [],
      feelings: [
        { depth: 1, feeling: "none" },
        { depth: 2, feeling: "secrets" },
      ],
      quests: [],
      ringGems: [],
      matchedRequirements: 0,
      totalRequirements: 0,
    };
    const html = renderToStaticMarkup(
      <ScoutPanel
        input={result.seed.code}
        onInput={() => {}}
        onScout={() => {}}
        loading={false}
        result={result}
        renderedChallenges={["darkness"]}
      />,
    );
    expect(html).toContain("1 challenge");
    expect(html).toContain('aria-label="Show floor 1 map"');
    expect(html).toContain('aria-label="Show floor 2 map"');
    expect(html).toContain("No notable items on this floor.");
    expect(html.match(/aria-expanded="false"/g)).toHaveLength(2);
    expect(html.match(/hidden=""/g)).toHaveLength(2);
  });
});
