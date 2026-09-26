// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { MapItemTooltip } from "./MapItemTooltip";
import type { MapItemTooltip as Tooltip } from "./types";

let host: HTMLDivElement;
let root: Root;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.unstubAllGlobals();
});
async function show(items: Tooltip["items"]) {
  await act(async () =>
    root.render(
      <MapItemTooltip
        id="item"
        tip={{ cell: 1, label: "Locked chest", hidden: false, items }}
        x={10}
        y={10}
        width={600}
        height={400}
        onPointerEnter={() => {}}
      />,
    ),
  );
}
const sword = {
  name: "Shortsword",
  description: "A quite short sword.",
  image: 101,
  quantity: 1,
  deterministic: true,
};
it("shows the generated upgrade, enchantment and curse independently, with the shared pulse", async () => {
  await show([
    {
      ...sword,
      name: "Shocking Shortsword",
      upgrade: 2,
      cursed: true,
      enchantment: "Shocking",
      glow: { color: [255, 255, 255], periodMs: 500 },
    },
  ]);
  const chip = host.querySelector('[aria-label="Upgrade +2"]');
  expect(chip?.classList.contains("d1-chip-tag-up")).toBe(true);
  expect(chip?.classList.contains("d1-chip-tag")).toBe(true);
  expect(host.querySelector("strong")?.textContent).toBe("Shocking Shortsword");
  expect(host.querySelector(".d1-chip-tag-soft")).toBeNull();
  expect(host.textContent).toContain("Cursed");
  const glow = host.querySelector<HTMLElement>(".d1-sprite-glow")!;
  expect(glow.style.animationDuration).toBe("1s");
  expect(glow.style.backgroundColor).toBe("rgb(255 255 255)");
  expect(glow.style.maskImage).toContain("items.png");
});
it("keeps the identity glyph outside curse tint and names the specific curse", async () => {
  await show([
    {
      ...sword,
      name: "Mail Armor of Displacement",
      icon: [0, 0, 5, 5],
      upgrade: 0,
      cursed: true,
      curse: "Displacement",
      glow: { color: [0, 0, 0], periodMs: 1000 },
    },
  ]);
  expect(host.querySelector('[aria-label="Upgrade +0"]')).toBeNull();
  expect(host.textContent).toContain("Mail Armor of Displacement");
  expect(host.textContent).toContain("Cursed");
  const sprite = host.querySelector(".d1-map-item-icon")!;
  expect(sprite.children).toHaveLength(2);
  expect(sprite.children[1].querySelector(".d1-sprite-glow")).toBeNull();
  expect(host.querySelector<HTMLElement>(".d1-sprite-glow")!.style.animationDuration).toBe("2s");
});
it("omits unknown properties and never invents a glow for cursed wands", async () => {
  await show([sword, { ...sword, name: "Wand of frost", upgrade: 1, cursed: true, glow: null }]);
  const entries = host.querySelectorAll("article");
  expect(entries[0].querySelector(".d1-chip-tag")).toBeNull();
  expect(entries[1].textContent).toContain("Cursed");
  expect(host.querySelector(".d1-sprite-glow")).toBeNull();
});
