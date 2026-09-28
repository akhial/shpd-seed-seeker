// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryDocument } from "./query";
import { queryStore } from "../../app/store";
import { ARCANE_RESIN_SPRITE, spriteBoxCss } from "../../shared/sprites/sprites";
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

const resinChip = () => host.querySelector<HTMLButtonElement>(".d1-resin-chip")!;

const menuItems = () =>
  [...host.querySelectorAll('[role="menu"] [role="menuitem"]')].map((item) => item.textContent);

async function openResinMenu() {
  await act(async () =>
    resinChip().dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, clientX: 10, clientY: 10 }),
    ),
  );
}

async function selectItem(value: string) {
  const select = host.querySelector<HTMLSelectElement>(".d1-modal select")!;
  await act(async () => {
    select.value = value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

function checkbox(name: string) {
  const label = [...host.querySelectorAll("label")].find(
    (label) => label.textContent?.trim() === name,
  );
  expect(label, name).toBeDefined();
  return label!.querySelector<HTMLInputElement>("input")!;
}

async function toggle(name: string) {
  const input = checkbox(name);
  await act(async () => input.click());
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
  // The resin section starts from the query's resin, uncursed donors by default.
  expect(checkbox("Require uncursed wands").checked).toBe(true);
  await toggle("Limit wands to a floor");
  await click("Add Requirement");
  expect(toQueryDocument(queryStore.state)).toMatchObject({
    requirements: [],
    arcane_resin: 2,
    arcane_resin_filter: { max_depth: 4 },
  });
  expect(toQueryDocument(queryStore.state).arcane_resin_filter).toEqual({ max_depth: 4 });
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
  await openResinMenu();
  await click("Remove");
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

it("selects Auto, preserves filters, and restores the mode when editing", async () => {
  await render();
  await click("Add");
  await click("Wand");
  await selectItem("arcane_resin");
  await click("Auto");
  expect(host.querySelector('input[aria-label="Minimum resin"]')).toBeNull();
  expect(host.querySelector(".d1-modal")!.textContent).toContain("each kept wand to +3");
  expect(checkbox("Require uncursed wands").checked).toBe(true);
  await toggle("Limit wands to a floor");
  await click("Add Requirement");
  expect(toQueryDocument(queryStore.state)).toMatchObject({ arcane_resin: "auto" });
  expect(toQueryDocument(queryStore.state).arcane_resin_filter).toEqual({ max_depth: 4 });
  expect(host.querySelector(".d1-resin-chip")!.textContent).toContain("Auto");
  expect(host.querySelector(".d1-resin-chip")!.textContent).not.toContain("≥");
  expect(host.textContent).toContain("1 requirement");
  await click("Edit Arcane Resin");
  expect(
    host.querySelector('[aria-label="Resin amount mode"] [aria-pressed="true"]')!.textContent,
  ).toBe("Auto");
  await click("Amount");
  expect(host.querySelector<HTMLInputElement>('input[aria-label="Minimum resin"]')!.value).toBe(
    "2",
  );
  await click("Save Changes");
  expect(queryStore.state.arcaneResin).toBe(2);
  expect(host.querySelector(".d1-resin-chip")!.textContent).toContain("≥2");
});

it("preserves the Mage credit while switching resin modes", async () => {
  queryStore.setState(() => fromQueryJson('{"arcane_resin":"auto","requirements":[]}'));
  await render();
  await click("Edit Arcane Resin");
  await toggle("Include Mage’s starting wand");
  await click("Save Changes");
  expect(toQueryDocument(queryStore.state).arcane_resin_filter).toEqual({
    include_mage_wand: true,
  });
  expect(host.querySelector(".d1-resin-chip")!.textContent).toContain("Mage +2");
  await click("Edit Arcane Resin");
  await click("Amount");
  await click("Save Changes");
  expect(queryStore.state.arcaneResin).toBe(2);
  expect(queryStore.state.arcaneResinFilter?.includeMageWand).toBe(true);
});

it("excludes a reserved wand from Auto and clears the flag when changing its kind", async () => {
  await render();
  await click("Add");
  await click("Wand");
  await selectItem("wand_lightning");
  await toggle("Exclude from Auto resin");
  await click("Add Requirement");
  expect(toQueryDocument(queryStore.state).requirements[0]).toMatchObject({ exclude_resin: true });
  expect(host.textContent).toContain("No resin");
  const chip = host.querySelector<HTMLButtonElement>(".d1-chip")!;
  await act(async () =>
    chip.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })),
  );
  await click("Ring");
  expect(host.querySelector(".d1-modal")!.textContent).not.toContain("Exclude from Auto resin");
  await click("Save Changes");
  expect(queryStore.state.requirements[0].excludeResin).toBeUndefined();
});

it("removes resin through its chip menu, with no remove button of its own", async () => {
  queryStore.setState(() =>
    fromQueryJson('{"arcane_resin":"auto","requirements":[{"kind":"wand"}]}'),
  );
  await render();
  const chip = resinChip();
  expect(chip.getAttribute("aria-label")).toBe("Edit Arcane Resin");
  expect(chip.querySelectorAll("button")).toHaveLength(0);
  expect(host.querySelector('[aria-label="Remove Arcane Resin"]')).toBeNull();
  // Right-click: the resin stacks, joins and detaches nothing.
  await openResinMenu();
  expect(menuItems()).toEqual(["Edit…", "Remove"]);
  await click("Edit…");
  expect(host.querySelector('[role="menu"]')).toBeNull();
  expect(host.querySelector('input[aria-label="Minimum resin"]')).toBeNull();
  expect(
    host.querySelector('[aria-label="Resin amount mode"] [aria-pressed="true"]')!.textContent,
  ).toBe("Auto");
  await click("Cancel");
  expect(host.querySelector(".d1-modal")).toBeNull();
  // The keyboard's menu key.
  await act(async () =>
    resinChip().dispatchEvent(new KeyboardEvent("keydown", { key: ".", bubbles: true })),
  );
  expect(menuItems()).toEqual(["Edit…", "Remove"]);
  await click("Remove");
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(queryStore.state.requirements).toHaveLength(1);
  expect(host.querySelector(".d1-resin-chip")).toBeNull();
  expect(host.querySelector('[role="menu"]')).toBeNull();
  expect(host.querySelector(".d1-modal")).toBeNull();
});

it("opens the resin menu on a long press without opening its sheet", async () => {
  vi.useFakeTimers();
  try {
    queryStore.setState(() => fromQueryJson('{"arcane_resin":6,"requirements":[]}'));
    await render();
    const chip = resinChip();
    chip.setPointerCapture = vi.fn();
    const touch = (type: string) =>
      new PointerEvent(type, {
        bubbles: true,
        pointerId: 1,
        pointerType: "touch",
        button: 0,
        clientX: 20,
        clientY: 20,
      });
    await act(async () => chip.dispatchEvent(touch("pointerdown")));
    await act(async () => vi.advanceTimersByTime(600));
    expect(menuItems()).toEqual(["Edit…", "Remove"]);
    await act(async () => chip.dispatchEvent(touch("pointerup")));
    await act(async () =>
      chip.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 })),
    );
    expect(host.querySelector(".d1-modal")).toBeNull();
    expect(menuItems()).toEqual(["Edit…", "Remove"]);
  } finally {
    vi.useRealTimers();
  }
});

it("removes the focused resin chip with Delete", async () => {
  queryStore.setState(() => fromQueryJson('{"arcane_resin":6,"requirements":[]}'));
  await render();
  await act(async () =>
    resinChip().dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true })),
  );
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(host.querySelector(".d1-resin-chip")).toBeNull();
});

it("loads an Auto chip and uses its label while dragging to remove", async () => {
  queryStore.setState(() =>
    fromQueryJson('{"arcane_resin":"auto","requirements":[{"kind":"wand"}]}'),
  );
  await render();
  vi.spyOn(document, "elementFromPoint").mockImplementation(() =>
    host.querySelector(".d1-delete-zone"),
  );
  const button = await startResinDrag("mouse", "Auto");
  await act(async () =>
    button.dispatchEvent(
      new PointerEvent("pointerup", {
        bubbles: true,
        pointerId: 1,
        pointerType: "mouse",
        clientX: 40,
        clientY: 60,
      }),
    ),
  );
  expect(queryStore.state.arcaneResin).toBeUndefined();
  expect(queryStore.state.requirements).toHaveLength(1);
});

async function startResinDrag(pointerType = "mouse", amountLabel = "≥6") {
  const button = resinChip();
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
  expect(host.querySelector(".d1-chip-ghost")!.textContent).toContain(amountLabel);
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
