// @vitest-environment happy-dom
import { readFile } from "node:fs/promises";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vite-plus/test";
import init, { scout } from "../../lib/wasm/pkg/seedfinder.js";
import type { ScoutResult } from "../../lib/wasm/types";
import { ScoutPanel } from "./ScoutPanel";

let host: HTMLDivElement;
let root: Root;
let result: ScoutResult;

beforeAll(async () => {
  await init({
    module_or_path: await readFile("src/lib/wasm/pkg/seedfinder_bg.wasm"),
  });
  result = JSON.parse(scout('{"seed":"ABC-DEF-GHI"}')) as ScoutResult;
});

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

async function render(value = result) {
  await act(async () =>
    root.render(
      <ScoutPanel
        input={value.seed.code}
        onInput={() => {}}
        onScout={() => {}}
        loading={false}
        result={value}
      />,
    ),
  );
}

async function click(label: string) {
  const button = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
  expect(button).not.toBeNull();
  await act(async () => button.click());
}

it("opens all 36 real-engine mappings from the scout summary and closes without changing the seed", async () => {
  await render();
  expect(document.querySelector("dialog")).toBeNull();
  await click("Seed information");
  const dialog = document.querySelector("dialog")!;
  expect(dialog.open).toBe(true);
  expect(dialog.textContent).toContain("ABC-DEF-GHI");
  expect(dialog.querySelectorAll("dt")).toHaveLength(36);
  for (const category of Object.values(result.itemMappings!)) {
    for (const entry of category) {
      expect(dialog.textContent).toContain(entry.name);
      expect(dialog.textContent).toContain(entry.appearance);
    }
  }
  expect(dialog.querySelectorAll('[style*="item_icons.png"]')).toHaveLength(0);
  await click("Close seed information");
  expect(document.querySelector("dialog")).toBeNull();
  expect(host.textContent).toContain("ABC-DEF-GHI");
  expect(document.body.style.overflow).toBe("");
  expect(document.activeElement).toBe(host.querySelector('[aria-label="Seed information"]'));
});

it("dismisses on Escape and seed navigation, and hides the control for legacy results", async () => {
  await render();
  await click("Seed information");
  await act(async () => {
    document.querySelector("dialog")!.dispatchEvent(new Event("cancel"));
  });
  expect(document.querySelector("dialog")).toBeNull();
  await click("Seed information");
  await render({ ...result, seed: { code: "AAA-AAA-AAA", value: 0 } });
  expect(document.querySelector("dialog")).toBeNull();
  await render({ ...result, itemMappings: undefined });
  expect(host.querySelector('[aria-label="Seed information"]')).toBeNull();
});
