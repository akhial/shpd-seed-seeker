// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { defaultQueryState, fromQueryJson, toQueryJson } from "../../lib/query";
import { queryStore } from "../../lib/store";
import { analyzeQuery, getEngineInfo } from "../../lib/wasm";
import { SearchCoordinator, searchStore } from "../../lib/search/coordinator";
import { initialCoordinatorState } from "../../lib/search/coordinator-state";
import type { EngineInfo } from "../../lib/wasm/types";
import App from "./App";

vi.mock("../../lib/wasm", () => ({
  analyzeQuery: vi.fn(),
  getEngineInfo: vi.fn(),
  decodeShareText: vi.fn(),
  formatSeedCode: vi.fn(),
  parseSeedCode: vi.fn(),
}));
vi.mock("./QueryPanel", () => ({ QueryPanel: () => null }));
vi.mock("./ResultsPanel", () => ({ ResultsPanel: () => null }));
vi.mock("./ScoutPanel", () => ({ ScoutPanel: () => null }));

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
  queryStore.setState(() => fromQueryJson('{"requirements":[{"item":"rat_skull"}]}'));
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

it("checks the latest query before a keyboard start, even before the estimate debounce", async () => {
  const start = vi.spyOn(SearchCoordinator.prototype, "start").mockImplementation(() => {});
  await act(async () => root.render(<App />));
  const duplicate = fromQueryJson('{"requirements":[{"item":"rat_skull"},{"item":"rat_skull"}]}');
  vi.mocked(analyzeQuery).mockResolvedValue({
    valid: true,
    impossible: true,
    probability: null,
    notes: ["Duplicate trinket"],
  });
  await act(async () => {
    queryStore.setState(() => duplicate);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true }));
  });
  expect(analyzeQuery).toHaveBeenLastCalledWith(toQueryJson(duplicate));
  expect(start).not.toHaveBeenCalled();

  vi.mocked(analyzeQuery).mockResolvedValue({
    valid: true,
    impossible: false,
    probability: 1,
    notes: [],
  });
  await act(async () => {
    queryStore.setState(() => fromQueryJson('{"requirements":[{"item":"rat_skull"}]}'));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true }));
  });
  expect(start).toHaveBeenCalledOnce();
});
