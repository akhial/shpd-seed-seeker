import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vite-plus/test";
import type { QueryDocument } from "../wasm/types";
import { SearchCoordinator, clearResults, loadImportedResults, searchStore } from "./coordinator";
import {
  canClearResults,
  initialCoordinatorState,
  type CoordinatorState,
} from "./coordinator-state";

/**
 * Enough of the Worker interface for the coordinator, which only constructs
 * workers and posts to them. Nothing ever answers, so every assertion here is
 * about the state the coordinator commits synchronously when a search starts —
 * which is exactly where the fresh/refine decision shows.
 */
interface StubMessage {
  type: string;
  seeds?: number[];
  queryJson?: string;
  baseQueryJson?: string;
  segments?: { startSeed: number; endSeedExclusive: number }[];
  requestId?: number;
}

class StubWorker {
  /** Every message posted to any worker this test, in order. Collected
   * statically because the filter worker is a module-level singleton: it
   * outlives the test that first created it. */
  static posted: StubMessage[] = [];
  static listeners: ((event: MessageEvent) => void)[] = [];
  constructor() {}
  addEventListener(_name: string, listener: (event: MessageEvent) => void): void {
    StubWorker.listeners.push(listener);
  }
  postMessage(message: StubMessage): void {
    StubWorker.posted.push(message);
  }
  terminate(): void {}
}

const postedTypes = () => StubWorker.posted.map((message) => message.type);

beforeAll(() => vi.stubGlobal("Worker", StubWorker));
afterAll(() => vi.unstubAllGlobals());
beforeEach(() => {
  StubWorker.posted = [];
  StubWorker.listeners = [];
});

const TOTAL = 1_000;
const baseQuery: QueryDocument = { requirements: [{ kind: "ring" }] };
const superset: QueryDocument = { requirements: [{ kind: "ring" }, { kind: "weapon" }] };
const unrelated: QueryDocument = { requirements: [{ kind: "wand" }] };

const match = (value: number) => ({ value, code: value.toString().padStart(9, "A") });

/** Leaves the store as a finished anchor run of `query` that covered the
 * first 400 of 1,000 seeds and found two matches, with the Target that
 * settleRun would have established from it. */
function seedFinishedRun(
  query: QueryDocument,
  state: "completed" | "cancelled" = "completed",
): void {
  searchStore.setState((current) => ({
    ...current,
    sessionId: 1,
    state,
    matches: [match(11), match(22)],
    queryJson: JSON.stringify(query),
    query,
    segments: [[{ startSeed: 0, endSeedExclusive: TOTAL }]],
    workerScanned: { 0: [400] },
    workerCount: 1,
    completedWorkers: 1,
    target: {
      query,
      matches: [match(11), match(22)],
    },
  }));
}

describe("implicit refine on start", () => {
  it("continues the previous run when the query only gained requirements", () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    coordinator.start(superset, 2);

    const state = searchStore.state;
    // The filter phase is the refine's first half: previous matches are being
    // re-verified, and they survive in the store until it succeeds.
    expect(state.state).toBe("running");
    expect(state.filtering).toBe(true);
    expect(state.refined).toEqual({ kept: 0, of: 2 });
    expect(state.matches.map((item) => item.value)).toEqual([11, 22]);
    expect(postedTypes()).toEqual(["filter"]);
  });

  it("continues the previous run when the query is unchanged", () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    coordinator.start(baseQuery, 2);

    const state = searchStore.state;
    // An unchanged query is a resume, not a rescan: the same filter phase
    // runs (it trivially keeps everything) and the previous coverage stands.
    expect(state.state).toBe("running");
    expect(state.filtering).toBe(true);
    expect(state.matches.map((item) => item.value)).toEqual([11, 22]);
    expect(state.segments).toEqual([[{ startSeed: 0, endSeedExclusive: TOTAL }]]);
    expect(state.workerScanned).toEqual({ 0: [400] });
    expect(StubWorker.posted).toEqual([
      {
        type: "filter",
        queryJson: JSON.stringify(baseQuery),
        baseQueryJson: JSON.stringify(baseQuery),
        seeds: [11, 22],
        requestId: expect.any(Number),
      },
    ]);
  });

  it("keeps the results across repeated cancel/start cycles with an unchanged query", () => {
    // The QA repro: a session must survive until the user presses Clear.
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery, "cancelled");

    for (let cycle = 0; cycle < 3; cycle += 1) {
      coordinator.start(baseQuery, 2);
      expect(searchStore.state.filtering).toBe(true);
      expect(searchStore.state.matches.map((item) => item.value)).toEqual([11, 22]);

      // Cancelling during the filter phase falls back to the finished run it
      // was continuing, matches and coverage intact.
      coordinator.cancel();
      expect(searchStore.state.state).toBe("cancelled");
      expect(searchStore.state.filtering).toBe(false);
      expect(searchStore.state.matches.map((item) => item.value)).toEqual([11, 22]);
      expect(searchStore.state.workerScanned).toEqual({ 0: [400] });
    }
    // Three filter phases, and never a fresh full-space scan.
    expect(postedTypes()).toEqual(["filter", "filter", "filter"]);
  });

  it("filters unrelated loaded seeds before starting a fresh scan", async () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    coordinator.start(unrelated, 2);
    expect(searchStore.state.filtering).toBe(true);
    expect(searchStore.state.matches.map((item) => item.value)).toEqual([11, 22]);
    const filter = StubWorker.posted[0];
    StubWorker.listeners[0]({
      data: {
        type: "filter:result",
        requestId: filter.requestId,
        resultJson: JSON.stringify([match(11)]),
      },
    } as MessageEvent);
    await vi.waitFor(() => expect(searchStore.state.filtering).toBe(false));
    expect(searchStore.state.matches.map((item) => item.value)).toEqual([11]);
    expect(
      searchStore.state.segments
        .flat()
        .reduce((sum, range) => sum + range.endSeedExclusive - range.startSeed, 0),
    ).toBe(TOTAL);
    expect(searchStore.state.target?.matches.map((item) => item.value)).toEqual([11, 22]);
    expect(
      StubWorker.posted
        .filter((m) => m.type === "search:start")
        .every((m) => m.queryJson === JSON.stringify(unrelated)),
    ).toBe(true);
  });

  it("starts an imported refinement with its original trinket policy", async () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    loadImportedResults([match(11), match(22)], baseQuery);
    coordinator.start(superset, 1);
    const filter = StubWorker.posted[0];
    StubWorker.listeners[0]({
      data: {
        type: "filter:result",
        requestId: filter.requestId,
        resultJson: JSON.stringify([match(11)]),
      },
    } as MessageEvent);
    await vi.waitFor(() => expect(searchStore.state.filtering).toBe(false));
    const scan = StubWorker.posted.find((m) => m.type === "search:start")!;
    expect(JSON.parse(scan.queryJson!)).toEqual(superset);
    expect(searchStore.state.query).toEqual(superset);
    expect(searchStore.state.total).toBe(TOTAL);
  });

  it("fans a large filter phase out across the worker pool in contiguous slices", () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    const bigSet = Array.from({ length: 40 }, (_, index) => match(index + 1));
    searchStore.setState((current) => ({
      ...current,
      matches: bigSet,
      target: { ...current.target!, matches: bigSet },
    }));
    coordinator.start(baseQuery, 2);

    // 40 seeds over 2 workers: two contiguous halves whose concatenation is
    // the original input order, so the survivors keep their discovery order.
    const posted = StubWorker.posted.filter((message) => message.type === "filter");
    expect(posted.map((message) => message.seeds)).toEqual([
      bigSet.slice(0, 20).map((item) => item.value),
      bigSet.slice(20).map((item) => item.value),
    ]);
  });

  it("checks the full pool when requirements are removed", () => {
    const coordinator = new SearchCoordinator(TOTAL);
    // Removing requirements still checks every retained seed.
    const targetQuery: QueryDocument = { requirements: [{ kind: "ring" }, { kind: "weapon" }] };
    seedFinishedRun(targetQuery);
    coordinator.start(baseQuery, 2);

    const state = searchStore.state;
    expect(state.state).toBe("running");
    expect(state.filtering).toBe(true);
    expect(state.refined).toEqual({ kept: 0, of: 2 });
    expect(StubWorker.posted).toEqual([
      {
        type: "filter",
        queryJson: JSON.stringify(baseQuery),
        baseQueryJson: JSON.stringify(targetQuery),
        seeds: [11, 22],
        requestId: expect.any(Number),
      },
    ]);
  });

  it("starts fresh once the results have been cleared", async () => {
    const coordinator = new SearchCoordinator(TOTAL);
    seedFinishedRun(baseQuery);
    expect(canClearResults(searchStore.state)).toBe(true);

    clearResults();
    expect(searchStore.state.state).toBe("idle");
    expect(searchStore.state.matches).toEqual([]);
    expect(searchStore.state.queryJson).toBe("");
    expect(searchStore.state.target).toBeUndefined();
    // The session counter survives so a late message from the cleared run is
    // still recognised as stale.
    expect(searchStore.state.sessionId).toBe(1);

    // Same query that refined a moment ago; with no base left it rescans.
    coordinator.start(superset, 1);
    await vi.waitFor(() => expect(searchStore.state.filtering).toBe(false));
    expect(searchStore.state.matches).toEqual([]);
    expect(postedTypes()).toEqual(["search:start"]);
  });
});

describe("clearing results", () => {
  const withState = (state: CoordinatorState["state"], matches = [match(1)]): CoordinatorState => ({
    ...initialCoordinatorState(TOTAL),
    state,
    matches,
  });

  it("is unavailable while a search owns the state, or with nothing to clear", () => {
    expect(canClearResults(withState("running"))).toBe(false);
    expect(canClearResults(withState("stopping"))).toBe(false);
    expect(canClearResults(initialCoordinatorState(TOTAL))).toBe(false);
  });

  it("is available for any finished or loaded result set", () => {
    for (const state of ["completed", "cancelled", "failed", "imported"] as const) {
      expect(canClearResults(withState(state))).toBe(true);
    }
    // A completed run that matched nothing is still worth clearing: it is the
    // base a later start would refine from.
    expect(canClearResults(withState("completed", []))).toBe(true);
  });

  it("leaves a running search untouched", () => {
    new SearchCoordinator(TOTAL);
    searchStore.setState((state) => ({
      ...state,
      state: "running",
      matches: [match(7)],
      queryJson: "{}",
    }));
    clearResults();
    expect(searchStore.state.state).toBe("running");
    expect(searchStore.state.matches).toHaveLength(1);
  });
});

it("filters before a fresh traversal when automatic selection is toggled", () => {
  const coordinator = new SearchCoordinator(TOTAL);
  const query: QueryDocument = {
    max_depth: 19,
    requirements: [{ item: "runic_blade", upgrade: 1, effect: "Grim" }],
  };
  seedFinishedRun(query);
  const target = searchStore.state.target;
  coordinator.start({ ...query, auto_apply_trinket: true }, 1);
  expect(searchStore.state.filtering).toBe(true);
  expect(searchStore.state.target).toBe(target);
  expect(postedTypes()).toEqual(["filter"]);
});

it("passes saved choices to filter workers when continuing the same policy", () => {
  const coordinator = new SearchCoordinator(TOTAL);
  const query: QueryDocument = {
    auto_apply_trinket: true,
    max_depth: 19,
    requirements: [{ item: "runic_blade", upgrade: 1, effect: "Grim" }],
  };
  seedFinishedRun(query);
  searchStore.setState((state) => ({
    ...state,
    target: {
      ...state.target!,
      matches: [
        { ...match(11), selectedTrinket: "parchment_scrap" },
        { ...match(22), selectedTrinket: null },
      ],
    },
  }));
  coordinator.start(query, 1);
  expect(StubWorker.posted[0]).toMatchObject({
    type: "filter",
    baseQueryJson: JSON.stringify(query),
    seeds: [11, 22],
    trinkets: ["parchment_scrap", null],
  });
});

it("checks the entire accumulated pool with each seed's original source", async () => {
  const coordinator = new SearchCoordinator(TOTAL);
  seedFinishedRun(baseQuery);
  loadImportedResults([{ ...match(33), selectedTrinket: null }], unrelated);
  coordinator.start(superset, 1);
  const filters = StubWorker.posted.filter((m) => m.type === "filter");
  expect(filters.map((m) => m.seeds)).toEqual([[11, 22], [33]]);
  expect(filters.map((m) => JSON.parse(m.baseQueryJson!))).toEqual([baseQuery, unrelated]);
  for (const filter of filters)
    StubWorker.listeners[0]({
      data: {
        type: "filter:result",
        requestId: filter.requestId,
        resultJson: "[]",
      },
    } as MessageEvent);
  await vi.waitFor(() => expect(searchStore.state.filtering).toBe(false));
  expect(searchStore.state.matches).toEqual([]);
  expect(searchStore.state.target?.matches.map((m) => m.value)).toEqual([11, 22, 33]);
  // A fully exhausted scan allows a second Search to finish after rechecking the pool.
  searchStore.setState((state) => ({
    ...state,
    state: "completed",
    segments: [],
    workerScanned: {},
  }));
  StubWorker.posted = [];
  coordinator.start(baseQuery, 1);
  expect(StubWorker.posted.filter((m) => m.type === "filter").flatMap((m) => m.seeds!)).toEqual([
    11, 22, 33,
  ]);
});
