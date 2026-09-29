// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import type { EditorChange, EditorForm, EditorSheet } from "../../../engine/types";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "../query";
import { queryStore } from "../../../app/store";
import { QueryPanel } from "../QueryPanel";
import { RequirementEditor } from "./RequirementEditor";
import { changeSheet, openSheet } from "./sheet";

let host: HTMLDivElement;
let root: Root;

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  queryStore.setState(defaultQueryState);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  queryStore.setState(defaultQueryState);
  vi.unstubAllGlobals();
});

async function render(json: string) {
  queryStore.setState(() => fromQueryJson(json));
  await act(async () =>
    root.render(
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
    ),
  );
}

const modal = () => host.querySelector<HTMLElement>(".d1-modal");

async function click(name: string, within: ParentNode = host) {
  const found = [...within.querySelectorAll<HTMLButtonElement>("button")].find(
    (element) => (element.getAttribute("aria-label") ?? element.textContent?.trim()) === name,
  );
  expect(found, name).toBeDefined();
  await act(async () => found!.click());
}

async function toggle(name: string) {
  const label = [...modal()!.querySelectorAll("label")].find(
    (element) => element.textContent?.trim() === name,
  );
  expect(label, name).toBeDefined();
  await act(async () => label!.querySelector("input")!.click());
}

async function openChip(name: string) {
  const chip = [...host.querySelectorAll<HTMLElement>('[data-drop="chip"]')].find(
    (element) => element.querySelector(".d1-chip-name")?.textContent === name,
  );
  expect(chip, name).toBeDefined();
  await act(async () =>
    chip!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })),
  );
}

it("draws the core's form and saves a stack with its copies' floor", async () => {
  await render('{"requirements":[{"kind":"weapon","item":"spear"}]}');
  await openChip("Spear");
  expect(modal()!.querySelector("h2")!.textContent).toBe("Edit Requirement");
  expect(modal()!.querySelector(".d1-modal-title p")!.textContent).toBe("Spear");
  // A named weapon has no tier filter.
  expect(modal()!.querySelector('[aria-label="Tier predicate"]')).toBeNull();
  // The count stepper takes the stack section's label.
  await click("One more", modal()!.querySelector('[aria-label="Total item count"]')!);
  expect(modal()!.querySelector(".d1-stepper-value")!.textContent).toBe("×2");
  await toggle("Limit the extra copies to a floor");
  expect(modal()!.textContent).toContain("Copies within first 4 floors");
  // The slider's name stays put while its reading moves.
  const floors = modal()!.querySelector<HTMLInputElement>('input[type="range"]')!;
  expect(floors.getAttribute("aria-label")).toBe("Copies within first");
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(floors, "4");
    floors.dispatchEvent(new Event("input", { bubbles: true }));
  });
  expect(modal()!.textContent).toContain("Copies within first 6 floors");
  expect(floors.getAttribute("aria-label")).toBe("Copies within first");
  expect(floors.getAttribute("aria-valuetext")).toBe("6");
  await click("Save Changes");
  expect(modal()).toBeNull();
  expect(toQueryDocument(queryStore.state).requirements).toEqual([
    { kind: "weapon", item: "spear" },
    { kind: "weapon", item: "spear", max_depth: 6 },
  ]);
});

it("ticks effects through the core and keeps an unchanged save from touching the query", async () => {
  await render('{"requirements":[{"kind":"armor"}]}');
  const state = queryStore.state;
  await openChip("Any armor");
  await click("Specific…");
  expect(modal()!.textContent).toContain("Tick the effects the item may carry");
  const glyph = modal()!.querySelector<HTMLInputElement>('[aria-label="Effects"] input')!;
  await act(async () => glyph.click());
  expect(modal()!.textContent).toContain("Matches any one of 1 effect.");
  await act(async () => glyph.click());
  await click("Any", modal()!.querySelector('[aria-label="Glyph filter"]')!);
  expect(modal()!.querySelector('[aria-label="Effects"]')).toBeNull();
  await click("Save Changes");
  expect(queryStore.state).toBe(state);
});

it("shows why a draft cannot be saved and keeps its button off", async () => {
  await render('{"requirements":[{"item":"rat_skull"}]}');
  await click("Add");
  await click("Trinket");
  const item = modal()!.querySelector<HTMLSelectElement>("select")!;
  await act(async () => {
    item.value = "rat_skull";
    item.dispatchEvent(new Event("change", { bubbles: true }));
  });
  expect(modal()!.querySelector('[role="alert"]')!.textContent).toBe(
    "This trinket is already required. Each trinket appears only once in the deck.",
  );
  const add = [...modal()!.querySelectorAll("button")].find(
    (element) => element.textContent === "Add Requirement",
  )!;
  expect(add.disabled).toBe(true);
});

it("draws a control only while the form shows it, with any caption it carries", () => {
  const open = openSheet(defaultQueryState(), { type: "new", blanket: false });
  if (!open.ok) throw new Error(open.error);
  const { form } = open.value;
  const html = (patch: Partial<EditorForm>) =>
    renderToStaticMarkup(
      <RequirementEditor
        sheet={{ ...open.value, form: { ...form, ...patch } }}
        onChange={() => {}}
        onSave={() => {}}
        onCancel={() => {}}
      />,
    );
  expect(html({})).toContain('aria-label="Category"');
  expect(html({ category: { ...form.category, visible: false } })).not.toContain(
    'aria-label="Category"',
  );
  expect(html({})).toContain("Any weapon</option>");
  expect(html({ item: { ...form.item, visible: false } })).not.toContain("Any weapon</option>");
  // The floor limit alone still gets its Details section.
  const floorOnly = html({
    effect: { ...form.effect, visible: false },
    uncursed: { ...form.uncursed, visible: false },
    source: { ...form.source, visible: false },
  });
  expect(floorOnly).toContain("Details");
  expect(floorOnly).toContain("Limit this item to a floor");
  const counting = {
    ...form.stack,
    count_levels: {
      ...form.stack.count_levels,
      visible: true,
      enabled: false,
      caption: "Levels count once per ring.",
      caption_visible: true,
    },
  };
  expect(html({ stack: counting })).toContain("Levels count once per ring.");
  expect(
    html({
      stack: { ...counting, count_levels: { ...counting.count_levels, caption_visible: false } },
    }),
  ).not.toContain("Levels count once per ring.");
});

/** A sheet the core opened on a new chip, then moved by `changes`. */
function sheetAfter(...changes: EditorChange[]): EditorSheet {
  let answer = openSheet(defaultQueryState(), { type: "new", blanket: false });
  for (const change of changes) if (answer.ok) answer = changeSheet(answer.value, change);
  if (!answer.ok) throw new Error(answer.error);
  return answer.value;
}

const markup = (sheet: EditorSheet) =>
  renderToStaticMarkup(
    <RequirementEditor sheet={sheet} onChange={() => {}} onSave={() => {}} onCancel={() => {}} />,
  );

it("shows a slider, the effect grid and a caption on the core's flags, not on their modes", () => {
  const { form, draft } = sheetAfter({ type: "set_category", value: "armor" });
  const html = (patch: Partial<EditorForm>) => markup({ draft, form: { ...form, ...patch } });
  expect(html({})).not.toContain('aria-label="Minimum tier"');
  expect(html({ tier: { ...form.tier, mode: "at_least", value_visible: false } })).not.toContain(
    'aria-label="Minimum tier"',
  );
  expect(html({ tier: { ...form.tier, mode: "at_least", value_visible: true } })).toContain(
    'aria-label="Minimum tier"',
  );
  expect(html({ upgrade: { ...form.upgrade, mode: "exact", value_visible: false } })).not.toContain(
    'aria-label="Exactly"',
  );
  expect(html({ upgrade: { ...form.upgrade, mode: "exact", value_visible: true } })).toContain(
    'aria-label="Exactly"',
  );
  expect(
    html({ effect: { ...form.effect, mode: "specific", choices_visible: false } }),
  ).not.toContain('aria-label="Effects"');
  expect(html({ effect: { ...form.effect, choices_visible: true } })).toContain(
    'aria-label="Effects"',
  );
  const limited = { ...form.transmutations, visible: true, enabled: true, caption: "Up to one." };
  expect(html({ transmutations: { ...limited, caption_visible: false } })).not.toContain(
    "Up to one.",
  );
  expect(html({ transmutations: { ...limited, caption_visible: true } })).toContain("Up to one.");
});

it("labels the sections and check boxes and shows their help texts in the core's words", () => {
  // Armor: the effect section is the Glyph.
  const armor = sheetAfter({ type: "set_category", value: "armor" });
  expect(armor.form.effect.label).toBe("Glyph");
  expect(markup(armor)).toContain('<span class="d1-field-label">Glyph</span>');
  expect(markup(armor)).toContain('aria-label="Glyph filter"');

  // An ordinary wand: resin exclusion, with its help under it.
  const wand = sheetAfter({ type: "set_category", value: "wand" });
  expect(wand.form.exclude_resin.caption).not.toBeNull();
  expect(markup(wand)).toContain(`<p class="d1-caption">${wand.form.exclude_resin.caption}</p>`);

  // A trinket: the choice at +3, with its help under it.
  const trinket = sheetAfter({ type: "set_category", value: "trinket" });
  expect(markup(trinket)).toContain(
    `<p class="d1-caption">${trinket.form.select_trinket.caption}</p>`,
  );

  // A ring stack: the combined level's help shows beside its switch, on or off.
  const stack = sheetAfter(
    { type: "set_category", value: "ring" },
    { type: "set_item", value: "ring_might" },
    { type: "set_count", value: 2 },
  );
  const { count_levels } = stack.form.stack;
  expect(count_levels).toMatchObject({ visible: true, enabled: false, caption_visible: true });
  expect(markup(stack)).toContain(`<p class="d1-caption">${count_levels.caption}</p>`);
  expect(markup(stack)).toContain(`<h3>${stack.form.stack.label}</h3>`);

  // The transmutation limit's help shows only while the limit is on.
  const limited = sheetAfter(
    { type: "set_category", value: "trinket" },
    { type: "set_transmutations_enabled", value: true },
  );
  expect(limited.form.transmutations.caption_visible).toBe(true);
  expect(markup(limited)).toContain(limited.form.transmutations.caption!);
  expect(markup(trinket)).not.toContain(limited.form.transmutations.caption!);
});

it("words the resin section, its choice, bounds and Mage switch as the core does", () => {
  const amount = sheetAfter(
    { type: "set_category", value: "wand" },
    { type: "set_item", value: "arcane_resin" },
  );
  const { resin } = amount.form;
  const html = markup(amount);
  expect(html).toContain(`<span class="d1-field-label">${resin.label}</span>`);
  for (const mode of resin.modes) expect(html).toContain(`>${mode.label}</button>`);
  expect(html).toContain(
    `aria-label="${resin.label}" min="${resin.min}" max="${resin.max}" step="1" value="${resin.amount}"`,
  );
  expect(html).not.toContain(resin.caption);
  expect(html).toContain(`<span>${resin.include_mage_wand.label}</span>`);
  expect(html).toContain(`<p class="d1-caption">${resin.include_mage_wand.caption}</p>`);

  const auto = sheetAfter(
    { type: "set_category", value: "wand" },
    { type: "set_item", value: "arcane_resin" },
    { type: "set_resin_auto", value: true },
  );
  // Auto's meaning shows in the amount field's place.
  expect(markup(auto)).toContain(`<p class="d1-caption">${auto.form.resin.caption}</p>`);
  expect(markup(auto)).not.toContain('type="number"');
});
