// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, toQueryDocument } from "../features/query/query";
import { queryStore } from "./store";
import { analyzeQuery, decodeShareText, getEngineInfo } from "../engine/wasm";
import { searchStore } from "../features/search/coordinator";
import { initialCoordinatorState } from "../features/search/coordinator-state";
import type { EngineInfo } from "../engine/types";
import App from "./App";

vi.mock("../engine/wasm", () => ({
  analyzeQuery: vi.fn(),
  getEngineInfo: vi.fn(),
  decodeShareText: vi.fn(),
  formatSeedCode: vi.fn(),
  parseSeedCode: vi.fn(),
}));
vi.mock("../features/query/QueryPanel", () => ({ QueryPanel: () => null }));
vi.mock("../features/results/ResultsPanel", () => ({ ResultsPanel: () => null }));
vi.mock("../features/scout/ScoutPanel", () => ({ ScoutPanel: () => null }));

let root: Root;
let host: HTMLDivElement;
beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.mocked(getEngineInfo).mockResolvedValue({
    totalSeeds: 100,
    shpdVersion: "test",
  } as EngineInfo);
  vi.mocked(analyzeQuery).mockResolvedValue({
    valid: true,
    impossible: false,
    probability: 1,
    notes: [],
  });
  searchStore.setState(() => initialCoordinatorState());
  queryStore.setState(defaultQueryState);
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  window.history.replaceState(null, "", "/");
  queryStore.setState(defaultQueryState);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("brings a share link's requirements in as the editor writes them", async () => {
  // What the engine decodes from a link whose ring stack is spelled as a
  // named anchor with a plain copy under stack label 3.
  vi.mocked(decodeShareText).mockResolvedValue(
    '{"requirements":[{"item":"wand_frost","kind":"wand"},{"identity_group":3,"item":"ring_might","kind":"ring"},{"identity_group":3,"kind":"ring"}]}',
  );
  window.history.replaceState(null, "", "/#q=QActQALqAEDFAEDA");
  await act(async () => root.render(<App />));
  expect(decodeShareText).toHaveBeenCalledOnce();
  expect(toQueryDocument(queryStore.state).requirements).toEqual([
    { kind: "wand", item: "wand_frost" },
    { kind: "ring", item: "ring_might" },
    { kind: "ring", item: "ring_might" },
  ]);
  expect(window.location.hash).toBe("");
});
