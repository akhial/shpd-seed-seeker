import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vite-plus/test";
import { itemsByCategory } from "../../shared/game/catalog";
import { fromQueryJson, toQueryDocument } from "./query";
import { validateQuery } from "./validation";
import type { EditorSheet, QueryState, ScoutItem } from "../../engine/types";
import { CatalystEntry } from "../scout/ScoutPanel";
import { editBoard, requirementBoardOf } from "./requirements/board";
import { RequirementEditor } from "./requirements/RequirementEditor";
import { openSheet, saveSheet } from "./requirements/sheet";

const boardOf = (query: QueryState) => {
  const answer = requirementBoardOf(query);
  if (!answer.ok) throw new Error(answer.error);
  return answer.value;
};
const sheetOn = (query: QueryState, key: number): EditorSheet => {
  const answer = openSheet(query, { type: "row", key });
  if (!answer.ok) throw new Error(answer.error);
  return answer.value;
};
const sheetHtml = (sheet: EditorSheet) =>
  renderToStaticMarkup(
    <RequirementEditor sheet={sheet} onChange={() => {}} onSave={() => {}} onCancel={() => {}} />,
  );

describe("offered trinket pilot", () => {
  it("round-trips a transmutation limit and includes initial offers", () => {
    const query = fromQueryJson(
      '{"requirements":[{"any_of":[{"item":"rat_skull","trinket_transmutations":13},{"item":"mimic_tooth"}]}]}',
    );
    const requirement = query.requirements[0];
    expect(requirement.trinketTransmutations).toBe(13);
    expect(fromQueryJson(JSON.stringify(toQueryDocument(query)))).toEqual(query);
    expect(validateQuery(query).valid).toBe(true);
    const chip = boardOf(query).items[0].chips[0];
    expect(chip.details).toContain("within 13 transmutations");
    expect(chip.tags).toContainEqual({ text: "Transmute ≤13", style: "plain", tooltip: null });
    const html = sheetHtml(sheetOn(query, requirement.key));
    expect(html).toContain("Allow transmutations");
    expect(html).not.toContain("After transmuting");
    expect(html).toContain("At most 13");
    expect(html).toContain("Matches an initial offer or any of the next 13 trinkets.");
    expect(html).not.toContain("Choose matching trinket at +3");
    for (const count of [-1, 14, 1.5, "1", null]) {
      expect(() =>
        fromQueryJson(
          JSON.stringify({ requirements: [{ item: "rat_skull", trinket_transmutations: count }] }),
        ),
      ).toThrow("trinket_transmutations");
    }
  });

  it("highlights matched transmutations and names their exact positions", () => {
    const order = itemsByCategory.trinket.map((item, index) => ({
      id: item.id,
      name: item.name,
      spriteIndex: item.sprite,
      matched: index === 4 || index === 16,
    }));
    const offers: ScoutItem[] = order.slice(0, 4).map((item) => ({
      ...item,
      category: "trinket",
      depth: 2,
      source: "heap",
      upgrade: 0,
      cursed: false,
      secret: false,
      effect: null,
      accessibility: { type: "independent" },
      matched: false,
    }));
    const html = renderToStaticMarkup(<CatalystEntry offers={offers} order={order} />);
    expect(html).toContain(`Transmutation #1: ${order[4].name}, matches requirement`);
    expect(html).toContain(`Transmutation #13: ${order[16].name}, matches requirement`);
    expect(html.match(/class="d1-trinket-match"/g)).toHaveLength(2);
    expect(html).toContain("Transmutation order · 1–13");
  });

  it("persists choosing a trinket through grouped queries and shows only four scout overrides", () => {
    const query = fromQueryJson(
      '{"requirements":[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"rat_skull"}]}]}',
    );
    expect(query.requirements[0].selectTrinket).toBe(true);
    expect(fromQueryJson(JSON.stringify(toQueryDocument(query)))).toEqual(query);
    const order = itemsByCategory.trinket.map((i) => ({
      id: i.id,
      name: i.name,
      spriteIndex: i.sprite,
    }));
    const offers: ScoutItem[] = order.slice(0, 4).map((i) => ({
      ...i,
      category: "trinket",
      depth: 2,
      source: "heap",
      upgrade: 0,
      cursed: false,
      secret: false,
      effect: null,
      accessibility: { type: "independent" },
      matched: false,
    }));
    const html = renderToStaticMarkup(
      <CatalystEntry
        offers={offers}
        order={order}
        selectedTrinket={order[1].id}
        onSelect={() => {}}
      />,
    );
    expect(html).not.toContain("<select");
    expect(html.match(/<button /g)).toHaveLength(4);
    expect(html).not.toContain("No Trinket");
    expect(html).not.toContain("Click a trinket");
    expect(html).not.toContain("Applied trinket");
    expect(html).toContain(`aria-label="Apply ${order[1].name} at +3" aria-pressed="true"`);
    expect(html).toContain("Applied +3");
    expect(html).not.toContain(`aria-label="Apply ${order[4].name}`);
    const cleared = renderToStaticMarkup(
      <CatalystEntry offers={offers} order={order} onSelect={() => {}} disabled />,
    );
    expect(cleared).not.toContain('aria-pressed="true"');
    expect(cleared.match(/disabled=""/g)).toHaveLength(4);
    expect(cleared).not.toContain("Applied +3");
  });

  it("preserves all 17 identities, draws four choices in deck order, and highlights the match", () => {
    const order = itemsByCategory.trinket
      .map((item) => ({
        id: item.id,
        name: item.name,
        spriteIndex: item.sprite,
      }))
      .reverse();
    const offers: ScoutItem[] = order
      .slice(0, 4)
      .reverse()
      .map((entry) => ({
        ...entry,
        category: "trinket",
        depth: 2,
        source: "locked_chest",
        upgrade: 0,
        cursed: false,
        secret: false,
        effect: null,
        accessibility: { type: "independent" },
        matched: entry.id === order[1].id,
      }));
    const html = renderToStaticMarkup(<CatalystEntry offers={offers} order={order} />);
    expect(order).toHaveLength(17);
    expect(html).toContain("Magical catalyst");
    expect(html).not.toContain("Initial choices when");
    expect(html).not.toContain("not initial offers");
    expect(html).not.toContain("d1-badge-match");
    expect(html).not.toContain("d1-trinket-number");
    expect(html.match(/class="d1-trinket-choice(?: d1-trinket-match)?"/g)).toHaveLength(4);
    expect(html.match(/class="d1-trinket-choice d1-trinket-match"/g)).toHaveLength(1);
    for (let index = 1; index < 4; index++) {
      expect(html.indexOf(order[index - 1].name)).toBeLessThan(html.indexOf(order[index].name));
    }
    const tail = html.slice(html.indexOf('class="d1-trinket-tail"'));
    expect(tail.match(/<li /g)).toHaveLength(13);
    for (let index = 5; index < 17; index++) {
      expect(tail.indexOf(order[index - 1].name)).toBeLessThan(tail.indexOf(order[index].name));
    }
    expect(html).toContain("width:48px");
    expect(tail).toContain("width:24px");
    expect(tail).toContain("image-rendering:pixelated");
    expect(tail).not.toMatch(/>\d+<\//);
    for (const trinket of itemsByCategory.trinket) {
      expect(trinket.name).toBe(trinket.name.replace(/\b\w/g, (letter) => letter.toUpperCase()));
    }
  });

  it("requires a named trinket and shows no details or wildcard controls", () => {
    const query = fromQueryJson(
      '{"requirements":[{"kind":"trinket","source":"locked_chest","max_depth":2}]}',
    );
    expect(boardOf(query).items[0].chips[0].title).toBe("Trinket");
    expect(boardOf(query).items[0].problem).not.toBeNull();
    // The sheet names the first trinket and shows no filters for it, but keeps
    // the source and floor limit the row carried (a trinket offer follows the
    // catalyst's placement, so the engine searches them).
    const sheet = sheetOn(query, 1);
    expect(sheet.form.item.value).toBe("rat_skull");
    const saved = saveSheet(query, sheet);
    if (!saved.ok || !("saved" in saved.value)) throw new Error("the sheet did not save");
    const [trinket] = saved.value.saved.requirements;
    expect(trinket).toMatchObject({ key: 1, kind: "trinket", item: "rat_skull" });
    expect(trinket.source).toBe("locked_chest");
    expect(trinket.maxDepth).toBe(2);
    const html = sheetHtml(sheet);
    expect(html).toContain('<p class="d1-mono">Rat Skull</p>');
    expect(html).toContain("Choose matching trinket at +3");
    expect(html).not.toContain("Any trinket");
    expect(html).not.toContain('value=""');
    expect(html).not.toContain("Details");
    expect(html).not.toContain("Source");
    expect(html).not.toContain("Limit this item");
    expect(html.match(/<option /g)).toHaveLength(17);
  });

  it("joins named trinkets into an OR group and persists them as offered predicates", () => {
    const state = fromQueryJson('{"requirements":[{"item":"mimic_tooth"},{"item":"rat_skull"}]}');
    const joined = editBoard(state, [{ type: "join", source: 2, target: 1 }]);
    if (!joined.ok) throw new Error(joined.error);
    state.requirements = joined.value.requirements;
    expect(validateQuery(state).valid).toBe(true);
    const doc = toQueryDocument(state);
    expect(doc.requirements).toHaveLength(1);
    expect(doc.requirements[0]).toHaveProperty("any_of");
    expect(boardOf(state).items[0].chips[0].stack.can_grow).toBe(false);
    expect(
      fromQueryJson(JSON.stringify(doc))
        .requirements.map((r) => r.item)
        .sort((left, right) => (left ?? "").localeCompare(right ?? "")),
    ).toEqual(["mimic_tooth", "rat_skull"]);
  });
});
