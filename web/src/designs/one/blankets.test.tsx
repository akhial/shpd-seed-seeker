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
    '<details class="d1-details d1-blanket-details"><summary><span>Blanket Requirements</span><span class="d1-count">1</span></summary>',
  );
  expect(html).not.toContain("Each blanket must match at least one item");
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
