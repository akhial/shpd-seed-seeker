import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, expect, it } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryJson, validateQuery } from "./query";
import { queryStore } from "../../app/store";
import init, { analyze_query, SearchSession } from "../../engine/pkg/seedfinder.js";
import type { AnalysisResult } from "../../engine/types";
import { QueryPanel } from "./QueryPanel";
import { RequirementEditor } from "./requirements/RequirementEditor";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("../../engine/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});
afterEach(() => queryStore.setState(defaultQueryState));

const trinkets = [
  "parchment_scrap",
  "thirteen_leaf_clover",
  "rat_skull",
  "vial_of_blood",
  "petrified_seed",
  "wondrous_resin",
  "mimic_tooth",
];

it.each([0, 1, 2])("blocks oversubscribed offers and shared transmute limit %s", (limit) => {
  const requirements = trinkets.slice(0, 5 + limit).map((item, index) => ({
    item,
    trinket_transmutations: index < 4 ? 0 : limit,
  }));
  if (limit === 0) requirements.push({ item: "wondrous_resin", trinket_transmutations: 3 });
  const query = fromQueryJson(JSON.stringify({ requirements }));
  queryStore.setState(() => query);
  const json = toQueryJson(query);
  const analysis = JSON.parse(analyze_query(json)) as AnalysisResult;
  expect(analysis).toMatchObject({ valid: true, impossible: true, probability: null });
  if (!analysis.valid) throw new Error(analysis.error);
  expect(analysis.notes).toHaveLength(1);
  expect(analysis.notes[0]).toBe(
    limit === 0
      ? "Requires 5 initial trinket offers, but each seed offers only 4."
      : `${5 + limit} trinket requirements allow at most ${limit} transmutation${limit === 1 ? "" : "s"}; only ${4 + limit} distinct trinkets are reachable.`,
  );
  const html = renderToStaticMarkup(
    <QueryPanel
      analysis={analysis}
      validation={validateQuery(query)}
      running={false}
      engineReady
      onToggleSearch={() => {}}
      isMac={false}
      shareNotice={undefined}
      onDismissShareNotice={() => {}}
    />,
  );
  expect(html).toContain(analysis.notes[0]);
  expect(html).not.toContain("Quest-reward-only");
  expect(html).not.toContain("floor limit");
  expect(html).toMatch(/<button[^>]*disabled=""[^>]*><span>Start Search/);
  const session = new SearchSession(json, 0, 16);
  try {
    expect(JSON.parse(session.advance(16))).toMatchObject({
      state: "completed",
      tested: 0,
      matches: [],
    });
  } finally {
    session.free();
  }
});

it("uses the engine's floor-specific reason for a floor failure", () => {
  const analysis = JSON.parse(
    analyze_query('{"max_depth":6,"requirements":[{"item":"wand_lightning","upgrade":3}]}'),
  );
  expect(analysis.notes).toEqual([
    "Requirement 1 (Wand of lightning) needs floor 7 or later, beyond its floor limit.",
  ]);
});

it("blocks duplicate trinkets in saved queries and the editor, while allowing blanket reuse", () => {
  const query = fromQueryJson(
    '{"requirements":[{"item":"rat_skull"},{"item":"rat_skull","trinket_transmutations":13}]}',
  );
  expect(JSON.parse(analyze_query(toQueryJson(query)))).toMatchObject({
    impossible: true,
    notes: [
      "Rat Skull is required more than once, but each trinket appears only once in the deck.",
    ],
  });
  for (const blanket of [false, true]) {
    const html = renderToStaticMarkup(
      <RequirementEditor
        requirement={{ ...query.requirements[1], blanket }}
        isNew
        stack={{ count: 1, inCluster: false }}
        otherRequirements={[query.requirements[0]]}
        onSave={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(html.includes("This trinket is already required.")).toBe(!blanket);
  }
  const html = renderToStaticMarkup(
    <RequirementEditor
      requirement={query.requirements[0]}
      isNew={false}
      stack={{ count: 1, inCluster: false }}
      otherRequirements={[]}
      onSave={() => {}}
      onCancel={() => {}}
    />,
  );
  expect(html).not.toContain("This trinket is already required.");
});
