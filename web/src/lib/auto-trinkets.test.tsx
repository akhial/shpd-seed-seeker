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

it("searches one world, returns its recipe, and replays it in scouting and filtering", () => {
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

it("shows Performance on one core and defers to explicit trinket requirements", () => {
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
  expect(panel()).toContain("Performance");
  expect(panel()).toContain("Auto-apply a trinket at +3");
  expect(panel()).not.toContain("Workers");
  queryStore.setState(() =>
    fromQueryJson(
      '{"auto_apply_trinket":true,"requirements":[{"any_of":[{"item":"rat_skull"},{"item":"mimic_tooth"}]}]}',
    ),
  );
  const html = panel();
  expect(html).toContain("Uses your trinket requirements instead.");
  expect(html).toMatch(/<input type="checkbox" disabled="" checked=""\/><span>Auto-apply/);
});
