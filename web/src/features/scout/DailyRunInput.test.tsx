// @vitest-environment happy-dom
import { readFile } from "node:fs/promises";
import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vite-plus/test";
import init, { scout, level_map, parse_seed_code } from "../../engine/pkg/seedfinder.js";
import type { ScoutResult } from "../../engine/types";
import { DailyRunInput, todayUTC } from "./DailyRunInput";

let host: HTMLDivElement;
let root: Root;
const onScout = vi.fn();
beforeAll(async () => {
  await init({ module_or_path: await readFile("src/engine/pkg/seedfinder_bg.wasm") });
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
  const button = [...host.querySelectorAll("button")].find(
    (b) => (b.getAttribute("aria-label") ?? b.textContent) === label,
  )!;
  expect(button).toBeDefined();
  await act(async () => button.click());
}
const field = () =>
  host.querySelector<HTMLInputElement>('input[aria-label="Seed code or daily date"]')!;
async function enter(value: string, input = field()) {
  await act(async () => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  });
}
async function submit() {
  await act(async () =>
    host
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })),
  );
}
it("automatically formats seeds and partial dates in the same persistent text field", async () => {
  await act(async () => root.render(<Harness />));
  const original = field();
  for (const [typed, expected] of [
    ["a", "A"],
    ["abcdefgh", "ABC-DEF-GH"],
    ["abcdefghi", "ABC-DEF-GHI"],
    ["", ""],
    ["2", "2"],
    ["20260", "2026-0"],
    ["202609", "2026-09"],
    ["20260925", "2026-09-25"],
    ["2026-09-2", "2026-09-2"],
    ["abc", "ABC"],
  ]) {
    await enter(typed!);
    expect(field()).toBe(original);
    expect(field().value).toBe(expected);
    expect(field().type).toBe("text");
    expect(field().className).toBe("d1-seed-field d1-mono");
  }
  expect(
    [...host.querySelectorAll("button")].map(
      (button) => button.getAttribute("aria-label") ?? button.textContent,
    ),
  ).toEqual(["Choose daily run date", "Today", "Scout"]);
});
it("scouts Today directly and recomputes its date at UTC rollover", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-09-24T23:59:59Z"));
  await act(async () => root.render(<Harness />));
  await click("Today");
  expect(field().value).toBe("2026-09-24");
  expect(onScout).toHaveBeenLastCalledWith("2026-09-24");
  vi.setSystemTime(new Date("2026-09-25T00:00:00Z"));
  await click("Today");
  expect(field().value).toBe("2026-09-25");
  expect(onScout).toHaveBeenLastCalledWith("2026-09-25");
  expect(todayUTC()).toBe("2026-09-25");
});
it.each(["2024-02-29", "2030-01-01"])("scouts a picked past or future date: %s", async (date) => {
  await act(async () => root.render(<Harness initial="ABC-DEF-GHI" />));
  const picker = host.querySelector<HTMLInputElement>('input[type="date"]')!;
  Object.defineProperty(picker, "showPicker", { value: undefined });
  await click("Choose daily run date");
  expect(picker.getAttribute("aria-hidden")).toBe("false");
  await enter(date, picker);
  expect(field().value).toBe(date);
  expect(picker.getAttribute("aria-hidden")).toBe("true");
  await submit();
  expect(onScout).toHaveBeenCalledWith(date);
});
it("opens the native date selector when available", async () => {
  await act(async () => root.render(<Harness />));
  const showPicker = vi.fn();
  Object.defineProperty(host.querySelector('input[type="date"]'), "showPicker", {
    value: showPicker,
  });
  await click("Choose daily run date");
  expect(showPicker).toHaveBeenCalledOnce();
});
it.each(["2026-09-2", "2026-02-30", "1969-12-31", "ABC-DEF-GH"])(
  "keeps invalid or partial input editable without scouting: %s",
  async (input) => {
    await act(async () => root.render(<Harness initial={input} />));
    expect(field().value).toBe(input);
    expect(host.querySelector<HTMLButtonElement>('button[type="submit"]')!.disabled).toBe(true);
    await submit();
    expect(onScout).not.toHaveBeenCalled();
  },
);
it("disables run changes while scouting", async () => {
  await act(async () => root.render(<Harness initial="2030-01-01" loading />));
  expect(
    [...host.querySelectorAll("button, input")].every((el) => (el as HTMLInputElement).disabled),
  ).toBe(true);
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
