// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { LevelMapView } from "./LevelMapView";
import { mapRequestJson, requestLevelMap } from "../../lib/level-map/client";
import type { LevelMapRequest, MapBundle } from "../../lib/level-map/types";

vi.mock("../../lib/level-map/client", () => ({
  mapRequestJson: (r: LevelMapRequest) =>
    JSON.stringify({
      seed: r.seed,
      depth: r.depth,
      branch: r.branch ?? 0,
      challenges: [...r.challenges].sort(),
      trinket: r.selectedTrinket ?? "none",
    }),
  requestLevelMap: vi.fn(),
}));
vi.mock("../../lib/level-map/render", () => ({
  createLevelMapRenderer: () => ({ animated: false, draw: vi.fn() }),
}));
vi.mock("../../lib/level-map/particles", () => ({
  createMapParticleRenderer: () => ({ animated: false, draw: vi.fn() }),
}));

let host: HTMLDivElement;
let root: Root;
let pending: Map<string, { resolve: (bundle: MapBundle) => void; reject: (error: Error) => void }>;
const props = {
  seed: "AAA-AAA-AAA",
  depth: 12,
  challenges: [],
  selectedTrinket: "none",
  floors: [{ depth: 12 }, { depth: 13 }],
};
function bundle(branches = [1]): MapBundle {
  return {
    textures: new Map(),
    map: {
      format: "seed-seeker-level-map",
      schemaVersion: 3,
      seed: props.seed,
      depth: 12,
      branch: 0,
      challenges: [],
      selectedTrinket: null,
      terrain: [],
      entrance: 0,
      exit: 1,
      traps: [],
      assets: [],
      width: 32,
      height: 32,
      branches: branches.map((branch) => ({
        branch,
        depth: 12,
        kind: "blacksmith_crystal",
        entrance: 0,
      })),
      kind: "regular",
      secretRooms: [[0, 0, 1, 1]],
      secretDoors: [],
      secretTraps: [],
      scene: { tileSize: 16, sprites: [], layers: [], concealedLayers: [] },
    },
  };
}
async function render(selectedTrinket = "none", depth = 12) {
  await act(async () =>
    root.render(<LevelMapView {...props} depth={depth} selectedTrinket={selectedTrinket} />),
  );
}
async function finish(depth = 12, trinket = "none", branch = 0, branches = [1]) {
  const request = pending.get(
    mapRequestJson({ ...props, depth, selectedTrinket: trinket, branch }),
  );
  expect(request).toBeDefined();
  await act(async () => request!.resolve(bundle(branches)));
}
function viewport(scope: ParentNode = host) {
  return scope.querySelector<HTMLDivElement>(".d1-map-viewport")!;
}
function transform(view = viewport()) {
  const canvas = view.querySelector("canvas")!;
  return [canvas.style.width, canvas.style.transform];
}
async function key(value: string, target: Element = viewport()) {
  await act(async () =>
    target.dispatchEvent(new KeyboardEvent("keydown", { key: value, bubbles: true })),
  );
}
async function click(text: string, scope: ParentNode = host) {
  const button = [...scope.querySelectorAll("button")].find((b) => b.textContent?.trim() === text)!;
  expect(button).toBeDefined();
  await act(async () => button.click());
}
async function zoomAndPan(target = viewport()) {
  await key("+", target);
  await key("+", target);
  await key("ArrowLeft", target);
  await key("ArrowDown", target);
}

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    clearRect() {},
  } as unknown as CanvasRenderingContext2D);
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private callback: ResizeObserverCallback) {}
      observe(target: Element) {
        this.callback(
          [{ target, contentRect: { width: 600, height: 350 } } as ResizeObserverEntry],
          this as unknown as ResizeObserver,
        );
      }
      disconnect() {}
    },
  );
  pending = new Map();
  vi.mocked(requestLevelMap).mockImplementation(
    (r) => new Promise((resolve, reject) => pending.set(mapRequestJson(r), { resolve, reject })),
  );
  host = document.createElement("div");
  document.body.append(host);
  root = createRoot(host);
});
afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

it("preserves the mounted viewport, zoom and pan through trinket loading, failures and retries", async () => {
  await render();
  await finish();
  await zoomAndPan();
  const view = viewport(),
    before = transform();
  await render("mimic_tooth");
  expect(viewport()).toBe(view);
  expect(transform()).toEqual(before);
  await act(async () =>
    pending
      .get(mapRequestJson({ ...props, selectedTrinket: "mimic_tooth" }))!
      .reject(new Error("retry me")),
  );
  expect(host.textContent).toContain("Couldn’t load this map");
  expect(viewport()).toBe(view);
  expect(transform()).toEqual(before);
  await click("Try again");
  await finish(12, "mimic_tooth");
  expect(viewport()).toBe(view);
  expect(transform()).toEqual(before);
  await click("Secrets");
  expect(transform()).toEqual(before);
  await render("none");
  await finish();
  expect(transform()).toEqual(before);
});

it("retains the branch through trinket changes, then fits Main if the branch disappears", async () => {
  await render();
  await finish();
  await click("Blacksmith Mine");
  await finish(12, "none", 1);
  await zoomAndPan();
  const view = viewport(),
    before = transform();
  await render("mimic_tooth");
  await finish(12, "mimic_tooth");
  await finish(12, "mimic_tooth", 1);
  expect(viewport()).toBe(view);
  expect(transform()).toEqual(before);
  expect(host.querySelector('[aria-pressed="true"]')?.textContent).toBe("Blacksmith Mine");
  await render("none");
  await finish(12, "none", 0, []);
  await finish(12, "none", 0, []);
  expect(viewport()).not.toBe(view);
  expect(viewport().classList.contains("d1-map-zoomed")).toBe(false);
});

it("keeps inline and expanded floors and viewports independent, including trinket reloads", async () => {
  await render();
  await finish();
  await zoomAndPan();
  const inline = viewport(),
    before = transform(inline);
  await click("Expand");
  await finish();
  const dialog = host.querySelector("dialog")!;
  await key("j", dialog);
  await finish(13);
  const expanded = viewport(dialog);
  await zoomAndPan(expanded);
  const expandedBefore = transform(expanded);
  expect(inline.getAttribute("aria-label")).toBe("Floor 12 layout");
  expect(expanded.getAttribute("aria-label")).toBe("Floor 13 layout");
  await render("mimic_tooth");
  await finish(12, "mimic_tooth");
  await finish(13, "mimic_tooth");
  expect(viewport(dialog)).toBe(expanded);
  expect(transform(expanded)).toEqual(expandedBefore);
  expect(transform(inline)).toEqual(before);
  await key("k", dialog);
  await finish(12, "mimic_tooth");
  expect(viewport(dialog).classList.contains("d1-map-zoomed")).toBe(false);
  await click("Close", dialog);
  expect(viewport()).toBe(inline);
  expect(transform(inline)).toEqual(before);
  await render("mimic_tooth", 13);
  await finish(13, "mimic_tooth");
  expect(viewport()).not.toBe(inline);
  expect(viewport().classList.contains("d1-map-zoomed")).toBe(false);
});
