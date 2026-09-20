// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "../../lib/query";
import { queryStore } from "../../lib/store";
import { ARCANE_RESIN_SPRITE, spriteBoxCss } from "../../lib/sprites";
import { QueryPanel } from "./QueryPanel";

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

async function render() {
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

async function click(name: string) {
  const button = [...host.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => (button.getAttribute("aria-label") ?? button.textContent?.trim()) === name,
  );
  expect(button, name).toBeDefined();
  await act(async () => button!.click());
}

async function selectItem(value: string) {
  const select = host.querySelector<HTMLSelectElement>(".d1-modal select")!;
  await act(async () => {
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

async function toggle(name: string) {
  const label = [...host.querySelectorAll("label")].find(
    (label) => label.textContent?.trim() === name,
  )!;
  expect(label).toBeDefined();
  await act(async () => label.querySelector<HTMLInputElement>("input")!.click());
}

it("adds resin from the second wand option, edits its filters, and removes the chip", async () => {
  await render();
  await click("Add");
  await click("Wand");
  expect(
    [...host.querySelectorAll(".d1-modal select option")].slice(0, 2).map((o) => o.textContent),
  ).toEqual(["Any wand", "Arcane Resin"]);
  await selectItem("wand_lightning");
  await click("Exactly");
  await selectItem("arcane_resin");
  expect(host.querySelector(".d1-modal")!.textContent).not.toContain("Upgrade level");
  expect(host.querySelector(".d1-modal")!.textContent).not.toContain("Total item count");
  expect(host.querySelector('input[aria-label="Minimum resin"]')).not.toBeNull();
  await toggle("Require uncursed wands");
  await toggle("Limit wands to a floor");
  await click("Add Requirement");
  expect(toQueryDocument(queryStore.state)).toMatchObject({
    requirements: [],
    arcane_resin: 2,
    arcane_resin_filter: { max_depth: 4 },
  });
  const chip = host.querySelector(".d1-resin-chip")!;
  expect(chip.textContent).toContain("≥2");
  expect(chip.textContent).toContain("F≤4");
  expect(chip.querySelector<HTMLElement>(".d1-sprite > span")!.style.backgroundPosition).toBe(
    spriteBoxCss(ARCANE_RESIN_SPRITE, 18).inner.backgroundPosition,
  );
  await click("Edit Arcane Resin");
  expect(host.querySelector<HTMLInputElement>('input[aria-label="Minimum resin"]')!.value).toBe(
    "2",
  );
  await toggle("Require uncursed wands");
  await toggle("Limit wands to a floor");
  await click("Save Changes");
  expect(toQueryDocument(queryStore.state).arcane_resin_filter).toEqual({ uncursed: false });
  await click("Remove Arcane Resin");
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(queryStore.state.arcaneResinFilter).toBeUndefined();
  expect(host.querySelector(".d1-resin-chip")).toBeNull();
});

it("loads legacy resin and can turn it into an ordinary wand requirement", async () => {
  queryStore.setState(() => fromQueryJson('{"arcane_resin":6,"requirements":[]}'));
  await render();
  await click("Edit Arcane Resin");
  expect(host.querySelector<HTMLInputElement>(".d1-modal .d1-check input")!.checked).toBe(true);
  expect(host.querySelector<HTMLInputElement>('input[aria-label="Minimum resin"]')!.value).toBe(
    "6",
  );
  await selectItem("wand_lightning");
  expect(host.querySelector(".d1-modal")!.textContent).toContain("Upgrade level");
  expect(host.querySelector('input[aria-label="Minimum resin"]')).toBeNull();
  await click("Save Changes");
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(queryStore.state.requirements).toHaveLength(1);
  expect(queryStore.state.requirements[0].item).toBe("wand_lightning");
});

async function startResinDrag(pointerType = "mouse") {
  const button = host.querySelector<HTMLButtonElement>(".d1-resin-edit")!;
  button.setPointerCapture = vi.fn();
  await act(async () =>
    button.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        pointerId: 1,
        pointerType,
        button: 0,
        clientX: 20,
        clientY: 20,
      }),
    ),
  );
  await act(async () =>
    button.dispatchEvent(
      new PointerEvent("pointermove", {
        bubbles: true,
        pointerId: 1,
        pointerType,
        clientX: 40,
        clientY: 60,
      }),
    ),
  );
  expect(host.querySelector(".d1-resin-chip")!.classList.contains("d1-chip-dragging")).toBe(true);
  expect(host.querySelector(".d1-chip-ghost")!.textContent).toContain("Arcane Resin");
  expect(host.querySelector(".d1-chip-ghost")!.textContent).toContain("≥6");
  expect(host.querySelector(".d1-delete-zone")).not.toBeNull();
  return button;
}

it.each(["mouse", "touch"])(
  "drags resin to remove with %s without changing other requirements",
  async (pointerType) => {
    queryStore.setState(() =>
      fromQueryJson(
        '{"arcane_resin":6,"arcane_resin_filter":{"max_depth":4},"requirements":[{"item":"wand_lightning"}]}',
      ),
    );
    const requirements = queryStore.state.requirements;
    await render();
    vi.spyOn(document, "elementFromPoint").mockImplementation(() =>
      host.querySelector(".d1-delete-zone"),
    );
    const button = await startResinDrag(pointerType);
    await act(async () =>
      button.dispatchEvent(
        new PointerEvent("pointerup", {
          bubbles: true,
          pointerId: 1,
          pointerType,
          clientX: 40,
          clientY: 60,
        }),
      ),
    );
    expect(queryStore.state.arcaneResin).toBeUndefined();
    expect(queryStore.state.arcaneResinFilter).toBeUndefined();
    expect(queryStore.state.requirements).toEqual(requirements);
    expect(host.querySelector(".d1-resin-chip")).toBeNull();
    expect(host.querySelector(".d1-chip-ghost")).toBeNull();
    expect(host.querySelector(".d1-delete-zone")).toBeNull();
    expect(host.querySelector(".d1-modal")).toBeNull();
  },
);

it.each(["chip", "board", "outside", "cancel", "escape"])(
  "preserves resin after dragging onto %s without opening the editor",
  async (destination) => {
    queryStore.setState(() =>
      fromQueryJson('{"arcane_resin":6,"requirements":[{"item":"wand_lightning"}]}'),
    );
    const query = toQueryDocument(queryStore.state);
    await render();
    vi.spyOn(document, "elementFromPoint").mockImplementation(() =>
      destination === "outside"
        ? null
        : host.querySelector(`[data-drop="${destination === "chip" ? "chip" : "board"}"]`),
    );
    const button = await startResinDrag();
    expect(host.querySelector(".d1-drop-alternative")).toBeNull();
    expect(host.querySelector(".d1-ghost-alternative")).toBeNull();
    if (destination === "escape") {
      await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
    }
    await act(async () =>
      button.dispatchEvent(
        new PointerEvent(destination === "cancel" ? "pointercancel" : "pointerup", {
          bubbles: true,
          pointerId: 1,
          pointerType: "mouse",
          clientX: 40,
          clientY: 60,
        }),
      ),
    );
    // Browsers can synthesize a click after the captured drag is released.
    await act(async () =>
      button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 })),
    );
    expect(toQueryDocument(queryStore.state)).toEqual(query);
    expect(host.querySelector(".d1-modal")).toBeNull();
    expect(host.querySelector(".d1-chip-ghost")).toBeNull();
    expect(host.querySelector(".d1-delete-zone")).toBeNull();
    await click("Edit Arcane Resin");
    expect(host.querySelector(".d1-modal")).not.toBeNull();
  },
);
