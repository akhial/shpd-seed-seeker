import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { describe, expect, it } from "vite-plus/test";
import type { EditorChange, EditorResponse, EditorSheet, QueryState } from "../../../engine/types";
import { fromQueryJson, requirementFromRow, requirementToRow, toQueryJson } from "../query";
import { changeSheet, openSheet, saveSheet } from "./sheet";
import type { SheetSaved, SheetTarget } from "./sheet";

// The core's golden answers, whose saved rows the web's codec must read back.
const fixtures = join(
  import.meta.dirname,
  "../../../../../crates/seedfinder-core/tests/fixtures/editor",
);

function sheet(query: QueryState, target: SheetTarget, ...changes: EditorChange[]): EditorSheet {
  let answer = openSheet(query, target);
  for (const change of changes) if (answer.ok) answer = changeSheet(answer.value, change);
  if (!answer.ok) throw new Error(answer.error);
  return answer.value;
}

function saved(query: QueryState, open: EditorSheet): SheetSaved {
  const answer = saveSheet(query, open);
  if (!answer.ok) throw new Error(answer.error);
  if ("refused" in answer.value) throw new Error(answer.value.refused.form.errors.join(" "));
  return answer.value.saved;
}

describe("the requirement sheet bridge", () => {
  it("reads every row a sheet saves and writes it back unchanged", async () => {
    const names = (await readdir(fixtures)).filter((name) => name.startsWith("editor-"));
    let rows = 0;
    for (const name of names) {
      const { response } = JSON.parse(await readFile(join(fixtures, name), "utf8")) as {
        response: EditorResponse | { error: string };
      };
      if (!("saved" in response)) continue;
      rows += response.saved.rows.length;
      expect(response.saved.rows.map(requirementFromRow).map(requirementToRow), name).toEqual(
        response.saved.rows,
      );
    }
    expect(rows).toBeGreaterThan(0);
  });

  it("adds a ring stack counting levels, drawn and saved by the core", () => {
    const query = fromQueryJson('{"requirements":[{"kind":"wand"}]}');
    const open = sheet(
      query,
      { type: "new", blanket: false },
      { type: "set_category", value: "ring" },
      { type: "set_item", value: "ring_might" },
      { type: "set_count", value: 2 },
      { type: "set_count_levels", value: true },
    );
    expect(open.form).toMatchObject({ mode: "new", title: "Ring of Might", can_save: true });
    expect(open.form.stack.count_levels).toMatchObject({ visible: true, enabled: true });
    // Counting levels speaks for the upgrades.
    expect(open.form.upgrade.visible).toBe(false);
    const { requirements, changed, focus, resin } = saved(query, open);
    expect(changed).toBe(true);
    expect(resin).toBeUndefined();
    expect(requirements.map((requirement) => requirement.key)).toEqual([1, 2, 3]);
    expect(focus).toBe(2);
    const document = JSON.parse(toQueryJson({ ...query, requirements })) as {
      requirements: unknown[];
    };
    expect(document.requirements.slice(1)).toEqual([
      { kind: "ring", item: "ring_might", level_sum: { group: 1, at_least: 2 } },
      { kind: "ring", item: "ring_might", level_sum: { group: 1, at_least: 2 } },
    ]);
  });

  it("keeps the query's list when a chip is saved unchanged", () => {
    const query = fromQueryJson(
      '{"requirements":[{"kind":"wand","item":"wand_frost","upgrade":{"at_least":2}},{"kind":"wand","item":"wand_frost"}]}',
    );
    const { requirements, changed, focus } = saved(query, sheet(query, { type: "row", key: 1 }));
    expect(changed).toBe(false);
    expect(requirements).toBe(query.requirements);
    expect(focus).toBe(1);
  });

  it("refuses a save the core refuses, with its reasons", () => {
    const query = fromQueryJson('{"requirements":[{"item":"rat_skull"}]}');
    const open = sheet(
      query,
      { type: "new", blanket: false },
      { type: "set_category", value: "trinket" },
      { type: "set_item", value: "rat_skull" },
    );
    expect(open.form.can_save).toBe(false);
    const answer = saveSheet(query, open);
    expect(answer.ok && "refused" in answer.value && answer.value.refused.form.errors).toEqual([
      "This trinket is already required. Each trinket appears only once in the deck.",
    ]);
  });

  it("turns a wand into the query's Arcane Resin and the resin back into a wand", () => {
    const query = fromQueryJson(
      '{"arcane_resin":"auto","arcane_resin_filter":{"max_depth":9},"requirements":[{"kind":"wand"},{"item":"rat_skull"}]}',
    );
    // A wand sheet's resin section starts from the query's resin.
    const picked = sheet(
      query,
      { type: "row", key: 1 },
      { type: "set_item", value: "arcane_resin" },
      { type: "set_resin_auto", value: false },
      { type: "set_resin_amount", value: null },
    );
    expect(picked.form).toMatchObject({ resin_picked: true, title: "Arcane Resin", preview: null });
    expect(picked.form.floor_limit).toMatchObject({ enabled: true, value: 9 });
    expect(picked.form.errors).toEqual(["Enter an amount from 1 to 65535."]);
    const typed = changeSheet(picked, { type: "set_resin_amount", value: 5 });
    if (!typed.ok) throw new Error(typed.error);
    const toResin = saved(query, typed.value);
    expect(toResin.requirements.map((requirement) => requirement.item)).toEqual(["rat_skull"]);
    expect(toResin.resin).toEqual({
      arcaneResin: 5,
      arcaneResinFilter: { uncursed: true, maxDepth: 9 },
    });

    const resinQuery = { ...query, ...toResin.resin, requirements: toResin.requirements };
    const open = sheet(resinQuery, { type: "resin" });
    expect(open.form).toMatchObject({ mode: "edit", origin: { type: "resin" } });
    expect(open.form.resin).toMatchObject({ visible: true, auto: false, amount: 5 });
    // Saved as it was, the resin stays the query's own.
    expect(saved(resinQuery, open).resin).toBeUndefined();

    const toWand = saved(
      resinQuery,
      sheet(resinQuery, { type: "resin" }, { type: "set_item", value: "wand_frost" }),
    );
    expect(toWand.resin).toEqual({ arcaneResin: undefined, arcaneResinFilter: undefined });
    expect(toWand.requirements.map((requirement) => requirement.item)).toEqual([
      "rat_skull",
      "wand_frost",
    ]);
  });

  it("will not open a row the core cannot read", () => {
    const query = fromQueryJson('{"requirements":[{"kind":"wand"},{"item":"wand_of_wonders"}]}');
    expect(openSheet(query, { type: "row", key: 2 })).toEqual({
      ok: false,
      error: "This requirement cannot be read: unknown item 'wand_of_wonders'.",
    });
  });
});
