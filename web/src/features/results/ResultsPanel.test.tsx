// @vitest-environment happy-dom
import { readFile } from "node:fs/promises";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, toQueryDocument } from "../query/query";
import { decodeResultsFile } from "./results-file";
import { searchStore } from "../search/coordinator";
import { initialCoordinatorState } from "../search/coordinator-state";
import { queryStore } from "../../app/store";
import init from "../../engine/pkg/seedfinder.js";
import { ResultsPanel } from "./ResultsPanel";

let root: Root;
let host: HTMLDivElement;
let fixture: string;
const readText = vi.fn<() => Promise<string>>();

beforeAll(async () => {
  fixture = await readFile(
    "../crates/seedfinder-core/tests/fixtures/results-export-v1.json",
    "utf8",
  );
  await init({ module_or_path: await readFile("src/engine/pkg/seedfinder_bg.wasm") });
});
beforeEach(async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.spyOn(navigator, "clipboard", "get").mockReturnValue({ readText } as unknown as Clipboard);
  readText.mockReset().mockResolvedValue(fixture);
  queryStore.setState(defaultQueryState);
  searchStore.setState(() => initialCoordinatorState());
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
  await act(async () =>
    root.render(<ResultsPanel analysis={undefined} hasRequirements={false} onScout={() => {}} />),
  );
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  queryStore.setState(defaultQueryState);
  searchStore.setState(() => initialCoordinatorState());
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function pasteButton() {
  return host.querySelector<HTMLButtonElement>('[aria-label="Import results from clipboard"]')!;
}
async function paste() {
  await act(async () => pasteButton().click());
}

it("reads only on demand and imports the same JSON query and seeds as a file", async () => {
  expect(readText).not.toHaveBeenCalled();
  await paste();
  const imported = decodeResultsFile(fixture);
  expect(readText).toHaveBeenCalledOnce();
  expect(toQueryDocument(queryStore.state)).toEqual(imported.queryDocument);
  expect(searchStore.state.state).toBe("imported");
  expect(searchStore.state.matches.map((match) => match.code)).toEqual(imported.seeds);
  expect(searchStore.state.matches.map((match) => match.selectedTrinket)).toEqual(
    imported.trinkets,
  );
  expect(host.querySelector('[role="alert"]')).toBeNull();
});

it("still imports from a JSON file through the same path", async () => {
  const input = host.querySelector<HTMLInputElement>('input[type="file"]')!;
  Object.defineProperty(input, "files", {
    value: [new File([fixture], "results.json", { type: "application/json" })],
  });
  await act(async () => input.dispatchEvent(new Event("change", { bubbles: true })));
  expect(searchStore.state.matches.map((match) => match.code)).toEqual(
    decodeResultsFile(fixture).seeds,
  );
  expect(readText).not.toHaveBeenCalled();
});

it.each([
  ["empty", "  \n", "The clipboard has no text"],
  ["invalid", "{broken", ""],
  ["oversized", " ".repeat(2 * 1024 * 1024 + 1) + "{}", ""],
])(
  "preserves the current query and results for %s clipboard text",
  async (_name, text, message) => {
    await paste();
    const query = queryStore.state;
    const results = searchStore.state;
    readText.mockResolvedValueOnce(text);
    await paste();
    expect(host.querySelector('[role="alert"]')?.textContent).toContain(message);
    expect(queryStore.state).toBe(query);
    expect(searchStore.state).toBe(results);
  },
);

it("explains a denied clipboard read and keeps file import available", async () => {
  readText.mockRejectedValueOnce(new DOMException("Denied", "NotAllowedError"));
  await paste();
  expect(host.querySelector('[role="alert"]')?.textContent).toContain("Allow clipboard access");
  expect(
    host.querySelector<HTMLButtonElement>('[aria-label="Import results from a file"]')?.disabled,
  ).toBe(false);
  expect(searchStore.state.state).toBe("idle");
});

it("explains when the browser has no clipboard API", async () => {
  vi.spyOn(navigator, "clipboard", "get").mockReturnValue(undefined as unknown as Clipboard);
  await paste();
  expect(host.querySelector('[role="alert"]')?.textContent).toContain("Open this page over HTTPS");
  expect(readText).not.toHaveBeenCalled();
});

it.each(["running", "stopping"] as const)("disables imports while %s", async (state) => {
  await act(async () => searchStore.setState((current) => ({ ...current, state })));
  expect(pasteButton().disabled).toBe(true);
  expect(
    host.querySelector<HTMLButtonElement>('[aria-label="Import results from a file"]')?.disabled,
  ).toBe(true);
  expect(readText).not.toHaveBeenCalled();
});

it.each(["running", "stopping"] as const)(
  "rejects a pending read if the search becomes %s",
  async (state) => {
    let resolve!: (text: string) => void;
    readText.mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      }),
    );
    await paste();
    const query = queryStore.state;
    await act(async () => searchStore.setState((current) => ({ ...current, state })));
    const search = searchStore.state;
    await act(async () => resolve(fixture));
    expect(host.querySelector('[role="alert"]')?.textContent).toContain("stop it before importing");
    expect(queryStore.state).toBe(query);
    expect(searchStore.state).toBe(search);
  },
);
