import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, expect, it, vi } from "vite-plus/test";
import { QueryPanel } from "../designs/one/QueryPanel";
import { defaultQueryState, fromQueryJson, toQueryJson } from "./query";
import { queryStore } from "./store";
import init, {
  SearchSession,
  decode_share_text,
  encode_share_link,
  filter_seeds,
  parse_seed_code,
  scout,
} from "./wasm/pkg/seedfinder.js";
import type { ParsedSeed, ScoutResult, SearchAdvance } from "./wasm/types";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("./wasm/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});
afterEach(() => {
  vi.unstubAllGlobals();
  queryStore.setState(defaultQueryState);
});

const document = {
  auto_apply_trinket: true,
  max_depth: 19,
  requirements: [{ item: "runic_blade", upgrade: 1, effect: "Grim" }],
};

it("keeps the setting in share links and rejects malformed flags", () => {
  const state = fromQueryJson(JSON.stringify(document));
  const link = encode_share_link(toQueryJson(state));
  expect(fromQueryJson(decode_share_text(link))).toEqual(state);
  expect(() => fromQueryJson('{"requirements":[],"auto_apply_trinket":"yes"}')).toThrow(/boolean/);
  expect(fromQueryJson('{"requirements":[]}').autoApplyTrinket).toBe(false);
});

it("keeps a necessary trinket and replays its recipe in scouting and filtering", () => {
  const json = JSON.stringify(document);
  const seed = JSON.parse(parse_seed_code("SRU-YSU-QHS")) as ParsedSeed;
  const session = new SearchSession(json, seed.value, seed.value + 1);
  try {
    const found = JSON.parse(session.advance(1)) as SearchAdvance;
    expect(found.tested).toBe(1);
    expect(found.state).toBe("completed");
    expect(found.matches).toEqual([{ ...seed, selectedTrinket: "parchment_scrap" }]);
    const manifest = JSON.parse(
      scout(
        JSON.stringify({
          seed: seed.code,
          query: document,
          trinket: found.matches[0].selectedTrinket,
        }),
      ),
    ) as ScoutResult;
    expect(manifest.selectedTrinket).toBe("parchment_scrap");
    expect(manifest.matchedRequirements).toBe(1);
    expect(
      JSON.parse(filter_seeds(json, new Float64Array([seed.value]), '["parchment_scrap"]')),
    ).toEqual(found.matches);
    expect(JSON.parse(filter_seeds(json, new Float64Array([seed.value]), "[null]"))).toEqual([]);
  } finally {
    session.free();
  }
});

it("removes an unnecessary trinket and restores it when a refined query needs it", () => {
  const json = JSON.stringify(document);
  const seed = JSON.parse(parse_seed_code("EYY-RUL-LQG")) as ParsedSeed;
  const session = new SearchSession(json, seed.value, seed.value + 1);
  try {
    const found = JSON.parse(session.advance(1)) as SearchAdvance;
    expect(found.tested).toBe(1);
    expect(found.matches).toEqual([{ ...seed, selectedTrinket: null }]);
    const manifest = JSON.parse(
      scout(JSON.stringify({ seed: seed.code, query: document, trinket: "none" })),
    ) as ScoutResult;
    expect(manifest.selectedTrinket).toBeNull();
    expect(manifest.matchedRequirements).toBe(1);
    const narrowed = JSON.stringify({
      ...document,
      requirements: [...document.requirements, { item: "whip", effect: "Venomous" }],
    });
    const seeds = new Float64Array([seed.value]);
    expect(JSON.parse(filter_seeds(narrowed, seeds, "[null]"))).toEqual([]);
    const refined = JSON.parse(filter_seeds(narrowed, seeds, "[null]", json));
    expect(refined).toEqual([{ ...seed, selectedTrinket: "parchment_scrap" }]);
    const selected = JSON.parse(
      scout(
        JSON.stringify({
          seed: seed.code,
          query: JSON.parse(narrowed),
          trinket: "parchment_scrap",
        }),
      ),
    ) as ScoutResult;
    expect(selected.matchedRequirements).toBe(2);
    expect(JSON.parse(filter_seeds(json, seeds, '["parchment_scrap"]', narrowed))).toEqual(
      found.matches,
    );
  } finally {
    session.free();
  }
});

it("shows AutoTrinket enabled in Search scope on one core and defers to explicit requirements", () => {
  vi.stubGlobal("navigator", { hardwareConcurrency: 1 });
  const panel = () =>
    renderToStaticMarkup(
      <QueryPanel
        analysis={undefined}
        validation={{ valid: true, errors: [] }}
        running={false}
        engineReady
        isMac={false}
        onToggleSearch={() => {}}
        shareNotice={undefined}
        onDismissShareNotice={() => {}}
      />,
    );
  queryStore.setState(defaultQueryState);
  expect(panel()).toContain("Search scope");
  expect(panel()).toContain('<input type="checkbox" checked=""/><span>AutoTrinket</span>');
  expect(panel()).not.toContain("Performance");
  expect(panel()).not.toContain("Workers");
  queryStore.setState(() =>
    fromQueryJson(
      '{"auto_apply_trinket":true,"requirements":[{"any_of":[{"item":"rat_skull"},{"item":"mimic_tooth"}]}]}',
    ),
  );
  const html = panel();
  expect(html).toContain("Uses your trinket requirements instead.");
  expect(html).toMatch(/<input type="checkbox" disabled="" checked=""\/><span>AutoTrinket/);
});

it("returns explicit no-trinket recipes when the offers cannot help", () => {
  const query = JSON.stringify({
    auto_apply_trinket: true,
    max_depth: 2,
    requirements: [{ item: "leather_armor", upgrade: 1 }],
  });
  const session = new SearchSession(query, 0, 64);
  try {
    const found = JSON.parse(session.advance(64)) as SearchAdvance;
    expect(found.matches.length).toBeGreaterThan(0);
    expect(found.matches.every((match) => match.selectedTrinket === null)).toBe(true);
    const replay = JSON.parse(
      filter_seeds(
        query,
        new Float64Array(found.matches.map((match) => match.value)),
        JSON.stringify(found.matches.map(() => null)),
      ),
    );
    expect(replay).toEqual(found.matches);
  } finally {
    session.free();
  }
});
