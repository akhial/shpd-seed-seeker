import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, expect, it } from "vite-plus/test";
import {
  defaultQueryState,
  fromQueryJson,
  toQueryDocument,
  toQueryJson,
  validateQuery,
} from "../../lib/query";
import { decodeResultsFile, encodeResultsFile } from "../../lib/results-file";
import { queryStore } from "../../lib/store";
import init, {
  analyze_query,
  decode_share_text,
  encode_share_link,
  scout,
  filter_seeds,
} from "../../lib/wasm/pkg/seedfinder.js";
import type { ScoutResult } from "../../lib/wasm/types";
import { QueryPanel } from "./QueryPanel";
import { RequirementEditor } from "./RequirementEditor";
import {
  applyEdit,
  boardItems,
  canStack,
  joinAlternatives,
  replaceRequirementSection,
} from "./relations";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(
      new URL("../../lib/wasm/pkg/seedfinder_bg.wasm", import.meta.url),
    ),
  });
});
afterEach(() => queryStore.setState(defaultQueryState));

const document = {
  max_depth: 9,
  requirements: [
    { item: "wand_lightning", upgrade: { at_least: 2 } },
    { item: "wand_disintegration", upgrade: { at_least: 2 } },
    { item: "wand_frost", upgrade: { at_least: 2 } },
    { kind: "wand", upgrade: 3, source: "wandmaker_reward", blanket: true },
  ],
};
const state = () => fromQueryJson(JSON.stringify(document));

it("preserves blankets in saved queries, share links, and results files", () => {
  const query = state();
  expect(validateQuery(query).valid).toBe(true);
  expect(fromQueryJson(toQueryJson(query))).toEqual(query);
  expect(fromQueryJson(decode_share_text(encode_share_link(toQueryJson(query))))).toEqual(query);
  expect(decodeResultsFile(encodeResultsFile(toQueryDocument(query), [])).query).toEqual(query);
  expect(() => fromQueryJson('{"requirements":[{"kind":"wand","blanket":"true"}]}')).toThrow(
    /boolean/,
  );
});

it("shows a collapsed blanket board with a count and the existing source and upgrade controls", () => {
  const query = state();
  queryStore.setState(() => query);
  const html = renderToStaticMarkup(
    <QueryPanel
      analysis={undefined}
      validation={validateQuery(query)}
      running={false}
      engineReady
      onToggleSearch={() => {}}
      isMac={false}
      shareNotice={undefined}
      onDismissShareNotice={() => {}}
    />,
  );
  expect(html).toContain('aria-label="Blanket Requirements"');
  expect(html).toContain(
    '<details class="d1-details d1-blanket-details"><summary><span>Blanket Requirements</span><span class="d1-count">1</span>',
  );
  expect(html).toContain('aria-label="About blanket requirements"');
  expect(html).toContain('aria-describedby="blanket-requirements-help"');
  expect(html).toContain(
    'id="blanket-requirements-help" role="tooltip" class="d1-blanket-help-tooltip" hidden=""',
  );
  expect(html).toContain("Each blanket must match at least one item");
  const editor = renderToStaticMarkup(
    <RequirementEditor
      requirement={query.requirements[3]}
      isNew
      stack={{ count: 1, inCluster: false }}
      onSave={() => {}}
      onCancel={() => {}}
    />,
  );
  expect(editor).toContain("New Blanket Requirement");
  expect(editor).toContain("Wandmaker");
  expect(editor).toContain("Upgrade");
  expect(editor).not.toContain("Total item count");
  expect(editor).not.toContain("At least one item used by your ordinary requirements");
});

it("keeps identical blanket chips separate from item stacks and OR labels on the other board", () => {
  const query = fromQueryJson(
    '{"requirements":[{"item":"wand_frost"},{"item":"wand_frost","blanket":true},{"item":"wand_frost","blanket":true}]}',
  );
  expect(boardItems(query.requirements)).toHaveLength(3);
  expect(canStack(query.requirements, boardItems(query.requirements)[1])).toBe(false);
  expect(joinAlternatives(query.requirements, 0, 1)).toEqual(query.requirements);
  const ordinary = [
    { ...query.requirements[0], alternativeGroup: 1 },
    { ...query.requirements[0], item: "wand_lightning", alternativeGroup: 1 },
  ];
  const blankets = joinAlternatives(query.requirements.slice(1), 0, 1);
  const merged = replaceRequirementSection(
    [...ordinary, ...query.requirements.slice(1)],
    true,
    blankets,
  );
  expect(merged[0].alternativeGroup).not.toBe(merged[2].alternativeGroup);
  expect(validateQuery({ ...query, requirements: merged }).valid).toBe(true);
  const edited = applyEdit(merged, 2, { ...merged[2], source: "wandmaker_reward" }, 1, undefined);
  expect(edited[2].blanket).toBe(true);
  expect(edited[2].source).toBe("wandmaker_reward");
});

it("rejects a blanket without ordinary items and mixed ordinary/blanket alternatives", () => {
  const query = state();
  query.requirements = query.requirements.slice(3);
  expect(validateQuery(query).valid).toBe(false);
  expect(JSON.parse(analyze_query(toQueryJson(query))).valid).toBe(false);
  const mixed = fromQueryJson(
    '{"requirements":[{"any_of":[{"kind":"wand"},{"kind":"wand","blanket":true}]}]}',
  );
  expect(validateQuery(mixed).valid).toBe(false);
});

it("estimates narrowing without charging for a fourth wand", () => {
  const query = state();
  const narrowed = JSON.parse(analyze_query(toQueryJson(query)));
  query.requirements.pop();
  const base = JSON.parse(analyze_query(toQueryJson(query)));
  expect(narrowed).toMatchObject({ valid: true, impossible: false });
  expect(narrowed.probability).toBeGreaterThan(0);
  expect(narrowed.probability).toBeLessThan(base.probability);
});

it("warns and disables search when a +3 blanket conflicts with exact +2 wands", () => {
  const query = fromQueryJson(
    JSON.stringify({
      requirements: [
        { item: "wand_lightning", upgrade: 2 },
        { item: "wand_disintegration", upgrade: 2 },
        { kind: "wand", upgrade: 3, blanket: true },
      ],
    }),
  );
  queryStore.setState(() => query);
  const analysis = JSON.parse(analyze_query(toQueryJson(query)));
  expect(analysis).toMatchObject({ valid: true, impossible: true, probability: null });
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
  expect(html).toContain("Impossible query");
  expect(html).toMatch(/<button[^>]*disabled=""[^>]*><span>Start Search/);
});

it("searches and scouts a real Wandmaker reward as one item serving two conditions", () => {
  const seed = "AAA-AAA-AAA";
  const initial = JSON.parse(scout(JSON.stringify({ seed }))) as ScoutResult;
  const reward = initial.items.find((item) => item.source === "wandmaker_reward");
  expect(reward).toBeDefined();
  const query = {
    requirements: [
      { item: reward!.id },
      { kind: "wand", source: "wandmaker_reward", blanket: true },
    ],
  };
  const result = JSON.parse(scout(JSON.stringify({ seed, query }))) as ScoutResult;
  expect(result.matchedRequirements).toBe(2);
  expect(result.items.filter((item) => item.matched)).toHaveLength(1);
  expect(JSON.parse(filter_seeds(JSON.stringify(query), new Float64Array([0])))).toHaveLength(1);
});

it("keeps Auto resin on the ordinary board and preserves blankets through shared documents", () => {
  const query = fromQueryJson(JSON.stringify({ ...document, arcane_resin: "auto" }));
  expect(validateQuery(query).valid).toBe(true);
  expect(fromQueryJson(decode_share_text(encode_share_link(toQueryJson(query))))).toEqual(query);
  expect(decodeResultsFile(encodeResultsFile(toQueryDocument(query), [])).query).toEqual(query);
  queryStore.setState(() => query);
  const html = renderToStaticMarkup(
    <QueryPanel
      analysis={undefined}
      validation={validateQuery(query)}
      running={false}
      engineReady
      onToggleSearch={() => {}}
      isMac={false}
      shareNotice={undefined}
      onDismissShareNotice={() => {}}
    />,
  );
  expect(html.match(/aria-label="Edit Arcane Resin"/g)).toHaveLength(1);
  expect(html).toContain("Auto");
  const blanketBoard = html.slice(html.indexOf('aria-label="Blanket Requirements"'));
  expect(blanketBoard).not.toContain("d1-resin-chip");
});

it("analyzes Auto blanket witnesses quickly and scouts the same reserved wand", () => {
  const query = {
    arcane_resin: "auto",
    requirements: [
      { item: "wand_lightning", upgrade: 2 },
      { kind: "wand", upgrade: 2, blanket: true },
    ],
  };
  const started = performance.now();
  const analysis = JSON.parse(analyze_query(JSON.stringify(query)));
  expect(performance.now() - started).toBeLessThan(1000);
  expect(analysis).toMatchObject({ valid: true, impossible: false });
  const direct = JSON.parse(
    analyze_query(JSON.stringify({ ...query, requirements: query.requirements.slice(0, 1) })),
  );
  expect(analysis.probability).toBeCloseTo(direct.probability, 12);
  const wider = {
    arcane_resin: "auto",
    auto_apply_trinket: true,
    requirements: [
      ...Array.from({ length: 4 }, () => ({ kind: "wand" })),
      { kind: "wand", upgrade: { at_least: 1 }, uncursed: true, blanket: true },
    ],
  };
  const wideStarted = performance.now();
  const wideAnalysis = JSON.parse(analyze_query(JSON.stringify(wider)));
  expect(performance.now() - wideStarted).toBeLessThan(1000);
  expect(wideAnalysis).toMatchObject({ valid: true, impossible: false });
  expect(wideAnalysis.probability).toBeGreaterThan(0);
  const result = JSON.parse(scout(JSON.stringify({ seed: "AAA-AAA-AAS", query }))) as ScoutResult;
  expect(result.matchedRequirements).toBe(3);
  expect(result.totalRequirements).toBe(3);
  expect(JSON.parse(filter_seeds(JSON.stringify(query), new Float64Array([18])))).toHaveLength(1);
});
