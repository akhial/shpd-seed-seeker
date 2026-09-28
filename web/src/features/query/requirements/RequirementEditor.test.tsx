// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "../query";
import { queryStore } from "../../../app/store";
import { QueryPanel } from "../QueryPanel";

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
  await click("One more", modal()!.querySelector('[aria-label="How many of this"]')!);
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
