// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "../query";
import { queryStore } from "../../../app/store";
import type { ItemSource } from "../../../engine/types";
import { QueryPanel } from "../QueryPanel";
import { QueryPanelBoundary } from "../QueryPanelBoundary";
import { requirementBoard, requirementEditor } from "../../../engine/editor";
import { RequirementBoard } from "./RequirementBoard";

// The real envelope answers every request; one test makes it fail.
vi.mock("../../../engine/editor", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../../engine/editor")>();
  return { ...actual, requirementEditor: vi.fn(actual.requirementEditor) };
});

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
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function render(json: string) {
  queryStore.setState(() => fromQueryJson(json));
  await act(async () =>
    root.render(
      <QueryPanelBoundary>
        <QueryPanel
          analysis={undefined}
          validation={{ valid: true, errors: [] }}
          running={false}
          engineReady
          isMac={false}
          onToggleSearch={() => {}}
          shareNotice={undefined}
          onDismissShareNotice={() => {}}
        />
      </QueryPanelBoundary>,
    ),
  );
}

const chip = (name: string): HTMLElement => {
  const found = [...host.querySelectorAll<HTMLElement>('[data-drop="chip"]')].find(
    (element) => element.querySelector(".d1-chip-name")?.textContent === name,
  );
  expect(found, name).toBeDefined();
  return found!;
};

const button = (name: string): HTMLButtonElement | undefined =>
  [...host.querySelectorAll<HTMLButtonElement>("button")].find(
    (element) => (element.getAttribute("aria-label") ?? element.textContent?.trim()) === name,
  );

async function click(name: string) {
  const found = button(name);
  expect(found, name).toBeDefined();
  await act(async () => found!.click());
}

async function openMenu(name: string) {
  await act(async () =>
    chip(name).dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, clientX: 10, clientY: 10 }),
    ),
  );
}

const pointer = (type: string, x: number, y: number) =>
  new PointerEvent(type, {
    bubbles: true,
    pointerId: 1,
    pointerType: "mouse",
    button: 0,
    clientX: x,
    clientY: y,
  });

/**
 * Lifts `source` and holds it over `target`, as far as hit-testing goes; a
 * function finds a target that appears only once the drag has begun.
 */
async function dragOver(source: HTMLElement, target: Element | null | (() => Element | null)) {
  source.setPointerCapture = vi.fn();
  vi.spyOn(document, "elementFromPoint").mockImplementation(
    typeof target === "function" ? target : () => target,
  );
  await act(async () => source.dispatchEvent(pointer("pointerdown", 20, 20)));
  await act(async () => source.dispatchEvent(pointer("pointermove", 60, 60)));
}

async function release(source: HTMLElement) {
  await act(async () => source.dispatchEvent(pointer("pointerup", 60, 60)));
}

const status = () => host.querySelector(".d1-board-status")?.textContent;

async function press(element: HTMLElement, key: string) {
  await act(async () =>
    element.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true })),
  );
}

const badges = (name: string) =>
  [...chip(name).querySelectorAll(".d1-stack-badge")].map((badge) => badge.textContent);

it("steps a stack from its menu, which follows the stack's fresh count", async () => {
  await render('{"requirements":[{"kind":"ring","item":"ring_might","upgrade":2}]}');
  await openMenu("Ring of Might");
  expect(button("ΣCount levels together")).toBeUndefined();
  await click("One more");
  expect(queryStore.state.requirements).toHaveLength(2);
  expect(host.querySelector(".d1-chip-menu-count .d1-mono")!.textContent).toBe("2");
  await click("One more");
  expect(queryStore.state.requirements).toHaveLength(3);
  expect(host.querySelector(".d1-chip-menu-count .d1-mono")!.textContent).toBe("3");
  expect(button("One more")!.disabled).toBe(true);
  await click("ΣCount levels together");
  expect(badges("Ring of Might")).toEqual(["≤3", "Σ ≥ 3"]);
  expect(host.textContent).toContain("1 requirement");
});

it("bounds both count steppers by the core's count_max", async () => {
  const answer = requirementBoard({
    rows: [
      { key: 1, kind: "ring", item: "ring_might" },
      { key: 2, kind: "ring", item: "ring_might" },
    ],
  });
  if (!answer.ok) throw new Error(answer.error);
  const [item] = answer.value.items;
  expect(item.stack).toMatchObject({ count: 2, max: 3, can_grow: true, count_max: 3 });
  const draw = (count_max: number) =>
    act(async () =>
      root.render(
        <RequirementBoard
          items={[{ ...item, stack: { ...item.stack, count_max } }]}
          onEdits={() => ({ notice: null, rekeyed: [] })}
          onEdit={() => null}
          onAdd={() => {}}
        />,
      ),
    );
  await draw(3);
  await openMenu("Ring of Might");
  expect(button("One more")!.disabled).toBe(false);
  // A stack that may only shed copies: the core bounds it at its count.
  await draw(2);
  expect(button("One more")!.disabled).toBe(true);
  await click("×2");
  const inline = host.querySelector<HTMLElement>('.d1-stack-edit[aria-label="How many"]')!;
  expect(inline.querySelector<HTMLButtonElement>('[aria-label="One more"]')!.disabled).toBe(true);
});

it("styles the resin chip's credit apart from its filter, each tag with the core's hover text", async () => {
  await render(
    '{"arcane_resin":"auto","arcane_resin_filter":{"include_mage_wand":true,"max_depth":9},"requirements":[{"kind":"wand","item":"wand_frost","upgrade":3}]}',
  );
  const tags = (within: Element) =>
    [...within.querySelectorAll<HTMLElement>(".d1-chip-tag:not(.d1-chip-tag-soft)")].map((tag) => ({
      text: tag.textContent,
      className: tag.className,
      title: tag.getAttribute("title"),
    }));
  expect(tags(host.querySelector(".d1-resin-chip")!)).toEqual([
    {
      text: "Auto",
      className: "d1-chip-tag d1-chip-tag-credit",
      title:
        "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies",
    },
    {
      text: "Mage +2",
      className: "d1-chip-tag d1-chip-tag-credit",
      title: "Starting Magic Missile contributes 2 resin",
    },
    { text: "F≤9", className: "d1-chip-tag", title: null },
  ]);
  // A requirement chip's tags carry no hover text of their own.
  expect(tags(chip("Wand of Frost"))).toEqual([
    { text: "+3", className: "d1-chip-tag d1-chip-tag-up", title: null },
  ]);
});

it("steps a combined level down to off and back on at the core's default", async () => {
  await render('{"requirements":[{"kind":"ring","item":"ring_might"}]}');
  await openMenu("Ring of Might");
  await click("One more");
  await click("ΣCount levels together");
  expect(badges("Ring of Might")).toEqual(["≤2", "Σ ≥ 2"]);
  await click("Σ ≥ 2");
  await click("Lower total");
  expect(badges("Ring of Might")).toEqual(["≤2", "Σ ≥ 1"]);
  await click("Lower total");
  // Off: the stepper stays open at the core's "Σ ≥ 0", with nothing to lower.
  expect(badges("Ring of Might")).toEqual(["×2", "Σ ≥ 0"]);
  expect(button("Lower total")!.disabled).toBe(true);
  expect(toQueryDocument(queryStore.state).requirements).toEqual([
    { kind: "ring", item: "ring_might" },
    { kind: "ring", item: "ring_might" },
  ]);
  await click("Raise total");
  expect(badges("Ring of Might")).toEqual(["≤2", "Σ ≥ 2"]);
});

it("opens the menu and removes a chip from the keyboard, or by dropping it on the remove zone", async () => {
  await render('{"requirements":[{"kind":"wand"},{"kind":"ring"},{"kind":"armor"}]}');
  await press(chip("Any wand"), ".");
  expect(host.querySelector('[role="menu"]')).not.toBeNull();
  expect(button("Edit…")).toBeDefined();
  await press(chip("Any wand"), "Escape");
  expect(host.querySelector('[role="menu"]')).toBeNull();
  await press(chip("Any ring"), "Delete");
  expect(toQueryDocument(queryStore.state).requirements).toEqual([
    { kind: "wand" },
    { kind: "armor" },
  ]);
  const armor = chip("Any armor");
  await dragOver(armor, () => host.querySelector('[data-drop="delete"]'));
  // The remove zone appears once the drag has begun.
  await act(async () => armor.dispatchEvent(pointer("pointermove", 70, 70)));
  expect(host.querySelector(".d1-delete-zone-over")).not.toBeNull();
  expect(host.querySelector(".d1-ghost-delete")!.textContent).toBe("remove");
  await release(armor);
  expect(toQueryDocument(queryStore.state).requirements).toEqual([{ kind: "wand" }]);
  expect(host.querySelector('[data-drop="delete"]')).toBeNull();
});

it("says why a row the core cannot read won't open, and lets it be removed", async () => {
  await render('{"requirements":[{"kind":"wand","item":"wand_of_wonders"},{"kind":"ring"}]}');
  const unknown = chip("Unknown requirement");
  expect(unknown.classList.contains("d1-chip-error")).toBe(true);
  await press(unknown, "Enter");
  expect(host.querySelector(".d1-modal")).toBeNull();
  expect(status()).toBe("This requirement cannot be read: unknown item 'wand_of_wonders'.");
  expect(host.querySelector('[role="alert"]')).toBeNull();
  await openMenu("Unknown requirement");
  expect(button("Edit…")).toBeUndefined();
  expect(button("orEither/or with…")).toBeUndefined();
  await click("Remove");
  expect(toQueryDocument(queryStore.state).requirements).toEqual([{ kind: "ring" }]);
});

it("reports a sheet the editor cannot open in the query pane", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  await render('{"requirements":[{"kind":"wand"}]}');
  vi.mocked(requirementEditor).mockReturnValueOnce({ ok: false, error: "the editor trapped" });
  await press(chip("Any wand"), "Enter");
  expect(host.querySelector('[role="alert"]')!.textContent).toContain(
    "The requirements could not be shown: the editor trapped",
  );
  await click("Clear requirements");
  expect(queryStore.state.requirements).toEqual([]);
  expect(host.querySelector('[role="alert"]')).toBeNull();
});

it("refuses a drag across categories onto a stack, says why and keeps the query", async () => {
  await render(
    '{"requirements":[{"kind":"ring","item":"ring_might","upgrade":2},{"kind":"ring","item":"ring_might"},{"kind":"wand"}]}',
  );
  const query = queryStore.state;
  const wand = chip("Any wand");
  await dragOver(wand, chip("Ring of Might"));
  expect(chip("Ring of Might").classList.contains("d1-drop-refused")).toBe(true);
  expect(host.querySelector(".d1-ghost-alternative")).toBeNull();
  expect(status()).toBe("Copies can only be grouped with the same item type.");
  await release(wand);
  expect(queryStore.state).toBe(query);
  expect(status()).toBe("Copies can only be grouped with the same item type.");
});

it("joins by drag and by pick, and a lone chip stays put on the board", async () => {
  await render(
    '{"requirements":[{"kind":"wand"},{"kind":"wand","item":"wand_frost"},{"kind":"ring"}]}',
  );
  const frost = chip("Wand of Frost");
  await dragOver(frost, chip("Any wand"));
  expect(chip("Any wand").classList.contains("d1-drop-alternative")).toBe(true);
  expect(host.querySelector(".d1-ghost-alternative")!.textContent).toBe("or");
  await release(frost);
  expect(host.querySelector('[role="group"][aria-label="Any of 2"]')).not.toBeNull();
  expect(toQueryDocument(queryStore.state).requirements).toEqual([
    { any_of: [{ kind: "wand" }, { kind: "wand", item: "wand_frost" }] },
    { kind: "ring" },
  ]);

  const query = queryStore.state;
  const ring = chip("Any ring");
  await dragOver(ring, host.querySelector('[data-drop="board"]'));
  await release(ring);
  expect(queryStore.state).toBe(query);

  await openMenu("Any ring");
  await click("orEither/or with…");
  expect(status()).toBe("Either/or with… choose a chip");
  expect(chip("Any wand").classList.contains("d1-chip-pickable")).toBe(true);
  await act(async () =>
    chip("Wand of Frost").dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
    ),
  );
  expect(host.querySelector('[role="group"][aria-label="Any of 3"]')).not.toBeNull();
  await openMenu("Any ring");
  await click("On its own");
  expect(host.querySelector('[role="group"][aria-label="Any of 2"]')).not.toBeNull();
});

it("marks a chip whose requirement has a problem and explains it on hover", async () => {
  await render('{"requirements":[{"kind":"wand","max_depth":30}]}');
  const wand = chip("Any wand");
  expect(wand.classList.contains("d1-chip-error")).toBe(true);
  expect(wand.getAttribute("aria-label")).toBe("Any wand, any upgrade, floors 1–30");
  await act(async () => wand.focus());
  expect(host.querySelector(".d1-chip-pop-error")!.textContent).toBe(
    "Requirement floor must be 1 through 24.",
  );
});

it("offers to clear the requirements when the editor cannot draw them", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  queryStore.setState(() => ({
    ...fromQueryJson('{"requirements":[{"kind":"wand"}]}'),
    arcaneResin: 2,
    arcaneResinFilter: { uncursed: true, source: "nowhere" as ItemSource },
  }));
  await act(async () =>
    root.render(
      <QueryPanelBoundary>
        <QueryPanel
          analysis={undefined}
          validation={{ valid: true, errors: [] }}
          running={false}
          engineReady
          isMac={false}
          onToggleSearch={() => {}}
          shareNotice={undefined}
          onDismissShareNotice={() => {}}
        />
      </QueryPanelBoundary>,
    ),
  );
  expect(host.querySelector('[role="alert"]')!.textContent).toContain(
    "The requirements could not be shown",
  );
  await click("Clear requirements");
  expect(queryStore.state.requirements).toEqual([]);
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(host.textContent).toContain("Start Search");
});
