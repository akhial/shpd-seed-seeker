import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it } from "vite-plus/test";
import init from "../../lib/wasm/pkg/seedfinder.js";
import type { ScoutResult } from "../../lib/wasm/types";
import { FloorMapButton } from "./ScoutMapLens";
import { ScoutPanel } from "./ScoutPanel";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(
      new URL("../../lib/wasm/pkg/seedfinder_bg.wasm", import.meta.url),
    ),
  });
});

const emptyScout: ScoutResult = {
  seed: { code: "AAA-AAA-AAA", value: 0 },
  items: [],
  ringGems: [],
  quests: [],
  feelings: [],
  selectedTrinket: null,
  matchedRequirements: 0,
  totalRequirements: 0,
};

describe("scout map discovery", () => {
  it("keeps layouts discoverable when floors have no catalogued loot", () => {
    const html = renderToStaticMarkup(
      <ScoutPanel
        input="AAA-AAA-AAA"
        onInput={() => {}}
        onScout={() => {}}
        loading={false}
        result={emptyScout}
      />,
    );
    expect(html.match(/aria-label="View map of floor /g)).toHaveLength(20);
    expect(html).toContain('aria-label="View map of floor 1"');
    expect(html).toContain('aria-label="View map of floor 24"');
    for (const depth of [5, 10, 15, 20, 25, 26]) {
      expect(html).not.toContain(`aria-label="View map of floor ${depth}"`);
    }
    expect(html).not.toContain('role="dialog"');
  });

  it("offers a named keyboard action and exposes which inspector is open", () => {
    const html = renderToStaticMarkup(
      <FloorMapButton seed="AAA-AAA-AAA" depth={12} challenges={[]} active onOpen={() => {}} />,
    );
    expect(html).toContain('<button type="button"');
    expect(html).toContain('aria-haspopup="dialog"');
    expect(html).toContain('aria-expanded="true"');
    expect(html).toContain('aria-controls="d1-scout-map-lens"');
    expect(html).toContain("<span>Map</span>");
  });

  it("labels the rendered scout's challenge profile while another scout is loading", () => {
    const html = renderToStaticMarkup(
      <ScoutPanel
        input="AAA-AAA-AAB"
        onInput={() => {}}
        onScout={() => {}}
        loading
        result={emptyScout}
        challenges={["into_darkness", "barren_land"]}
      />,
    );
    expect(html).toContain("2 challenges");
    expect(html).toContain("AAA-AAA-AAA");
    expect(html).toContain("d1-manifest-loading");
  });
});
