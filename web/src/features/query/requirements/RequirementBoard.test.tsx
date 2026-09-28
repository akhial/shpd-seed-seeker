// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "../query";
import { queryStore } from "../../../app/store";
import type { ItemSource } from "../../../engine/types";
import { QueryPanel } from "../QueryPanel";
import { QueryPanelBoundary } from "../QueryPanelBoundary";

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

/** Lifts `source` and holds it over `target`, as far as hit-testing goes. */
async function dragOver(source: HTMLElement, target: Element | null) {
  source.setPointerCapture = vi.fn();
  vi.spyOn(document, "elementFromPoint").mockImplementation(() => target);
  await act(async () => source.dispatchEvent(pointer("pointerdown", 20, 20)));
  await act(async () => source.dispatchEvent(pointer("pointermove", 60, 60)));
}

async function release(source: HTMLElement) {
  await act(async () => source.dispatchEvent(pointer("pointerup", 60, 60)));
}

const status = () => host.querySelector(".d1-board-status")?.textContent;

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
  const badges = [...chip("Ring of Might").querySelectorAll(".d1-stack-badge")].map(
    (badge) => badge.textContent,
  );
  expect(badges).toEqual(["≤3", "Σ ≥ 3"]);
  expect(host.textContent).toContain("1 requirement");
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
