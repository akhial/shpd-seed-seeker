// @vitest-environment happy-dom
import { readFile } from "node:fs/promises";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vite-plus/test";
import init, { scout, level_map, parse_seed_code } from "../../lib/wasm/pkg/seedfinder.js";
import type { ScoutResult } from "../../lib/wasm/types";
import { DailyRunInput, todayUTC } from "./DailyRunInput";

let host: HTMLDivElement;
let root: Root;
const onScout = vi.fn();
beforeAll(async () => {
  await init({ module_or_path: await readFile("src/lib/wasm/pkg/seedfinder_bg.wasm") });
});
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  onScout.mockClear();
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
function Harness({ initial = "", loading = false }) {
  const [input, onInput] = useState(initial);
  return <DailyRunInput {...{ input, onInput, onScout, loading }} />;
}
async function click(label: string) {
  const button = [...host.querySelectorAll("button")].find((b) => b.textContent === label)!;
  expect(button).toBeDefined();
  await act(async () => button.click());
}
it("switches modes and recomputes Today at UTC rollover", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-24T23:59:59Z"));
  await act(async () => root.render(<Harness />));
  await click("Daily run");
  expect(host.querySelector<HTMLInputElement>('input[type="date"]')?.value).toBe("2026-09-24");
  vi.setSystemTime(new Date("2026-09-25T00:00:00Z"));
  await click("Today");
  expect(onScout).toHaveBeenLastCalledWith("2026-09-25");
  expect(todayUTC()).toBe("2026-09-25");
  await click("Seed code");
  expect(host.querySelector('input[aria-label="Seed code"]')).not.toBeNull();
  expect(host.querySelector('input[type="date"]')).toBeNull();
});
it.each(["2024-02-29", "2030-01-01"])("scouts a selected past or future date: %s", async (date) => {
  await act(async () => root.render(<Harness initial={date} />));
  await act(async () =>
    host
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
  );
  expect(onScout).toHaveBeenCalledWith(date);
});
it("disables run changes while scouting", async () => {
  await act(async () => root.render(<Harness initial="2030-01-01" loading />));
  expect(
    [...host.querySelectorAll("button, input")].every((el) => (el as HTMLInputElement).disabled),
  ).toBe(true);
});
it("keeps an eight-letter partial seed in seed-code mode", async () => {
  await act(async () => root.render(<Harness initial="ABC-DEF-GH" />));
  expect(host.querySelector('input[aria-label="Seed code"]')).not.toBeNull();
  expect(host.querySelector('input[type="date"]')).toBeNull();
});
it("keeps the full daily identity through the real WASM scout, maps and trinket changes", () => {
  const seed = "2026-09-25";
  expect(JSON.parse(parse_seed_code(seed))).toEqual({ code: seed, value: 7_219_798_078_976 });
  const world = JSON.parse(scout(JSON.stringify({ seed }))) as ScoutResult;
  expect(world.seed).toEqual({ code: seed, value: 7_219_798_078_976 });
  expect(world.items.length).toBeGreaterThan(0);
  const trinket = world.trinketOrder![0]!.id;
  const changed = JSON.parse(scout(JSON.stringify({ seed, trinket }))) as ScoutResult;
  expect(changed.seed).toEqual(world.seed);
  expect(changed.selectedTrinket).toBe(trinket);
  expect(changed.itemMappings).toEqual(world.itemMappings);
  const map = JSON.parse(level_map(JSON.stringify({ seed, depth: 1, trinket })));
  expect(map.seed).toBe(seed);
  expect(() => scout('{"seed":"2026-02-30"}')).toThrow();
});
