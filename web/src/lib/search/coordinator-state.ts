import type { ParsedSeed, QueryDocument } from "../wasm/types";
import type { SeedRange } from "./traversal";

export type SearchStatus =
  | "idle"
  | "running"
  | "stopping"
  | "completed"
  | "cancelled"
  | "failed"
  | "imported";
export interface RateSample {
  at: number;
  tested: number;
}
export interface RefineSummary {
  kept: number;
  of: number;
}

/** Every loaded or discovered seed, retained until Clear. */
export interface TargetState {
  query: QueryDocument;
  /** Every loaded or discovered seed, sorted by value. */
  matches: ParsedSeed[];
  sources?: Record<number, QueryDocument>;
}
export interface CoordinatorState {
  sessionId: number;
  state: SearchStatus;
  tested: number;
  total: number;
  rate: number;
  elapsed: number;
  /** Every unique match delivered so far, sorted by seed value. Never
   * truncated: matches beyond the display cap fall inside the scanned region
   * and must survive into a refine's filter set. */
  matches: ParsedSeed[];
  capped: boolean;
  /** Per-worker, per-segment scanned prefix lengths, aligned with
   * `segments`. */
  workerScanned: Record<number, number[]>;
  completedWorkers: number;
  workerCount: number;
  startedAt: number;
  rateSamples: RateSample[];
  /** Per-worker segment assignment of the active traversal, recorded so a
   * later refine can compute exactly which ranges remain unscanned. */
  segments: SeedRange[][];
  /** JSON of the query document these matches belong to. */
  queryJson: string;
  /** Set while the current results came from refining a previous run. */
  refined?: RefineSummary;
  /** Unique matches kept by the filter. Survivors count toward RESULT_CAP;
   * an already-full collection requests RESULT_CAP additional matches. */
  sessionBaseline: number;
  /** True while a refine is re-verifying the previous results and no worker
   * has started scanning yet. */
  filtering: boolean;
  error?: string;
  /** The query that produced `matches` (captured at search start or import). */
  query?: QueryDocument;
  /** Imported entries dropped as duplicates or beyond the result cap. */
  importedDropped?: number;
  /** The session's Target, if one has been established. */
  target?: TargetState;
}

export const RESULT_CAP = 1_024;
export const initialCoordinatorState = (total = 0): CoordinatorState => ({
  sessionId: 0,
  state: "idle",
  tested: 0,
  total,
  rate: 0,
  elapsed: 0,
  matches: [],
  capped: false,
  workerScanned: {},
  completedWorkers: 0,
  workerCount: 0,
  startedAt: 0,
  rateSamples: [],
  segments: [],
  queryJson: "",
  filtering: false,
  sessionBaseline: 0,
});

/** Whether the current run has delivered its per-session quota of new
 * matches; the coordinator stops the workers once it has. */
export function runSaturated(state: CoordinatorState): boolean {
  return (
    state.matches.length >=
    (state.sessionBaseline >= RESULT_CAP ? state.sessionBaseline + RESULT_CAP : RESULT_CAP)
  );
}

/**
 * Whether "Clear results" has anything to discard. A running or stopping
 * search owns the state — including the coverage bookkeeping a later refine
 * needs — so it is never cleared from underneath, and a state that is already
 * idle and empty has nothing to clear.
 */
export function canClearResults(state: CoordinatorState): boolean {
  if (state.state === "running" || state.state === "stopping") return false;
  return state.state !== "idle" || state.matches.length > 0 || state.target !== undefined;
}

export function mergeMatches(
  existing: ParsedSeed[],
  incoming: ParsedSeed[],
  cap = RESULT_CAP,
): { matches: ParsedSeed[]; capped: boolean } {
  // Deduplicate by seed value: a refined search may re-test a small overlap
  // around the previous stop position and rediscover a filtered survivor.
  // Nothing is evicted at the cap — the workers of a capped session are told
  // to stop, and every match they delivered belongs to the scanned region a
  // refine relies on. Only the display is limited to `RESULT_CAP`.
  const byValue = new Map<number, ParsedSeed>();
  for (const match of [...existing, ...incoming]) {
    if (!byValue.has(match.value)) byValue.set(match.value, match);
  }
  const unique = [...byValue.values()].sort((left, right) => left.value - right.value);
  return { matches: unique, capped: unique.length >= cap };
}

export function calculateRate(samples: RateSample[]): number {
  if (samples.length < 2) return 0;
  const first = samples[0];
  const last = samples[samples.length - 1];
  const seconds = (last.at - first.at) / 1_000;
  return seconds > 0 ? (last.tested - first.tested) / seconds : 0;
}

/** Imports replace the current view and add their entries to the retained pool. */
export function importedResultsState(
  state: CoordinatorState,
  matches: ParsedSeed[],
  query: QueryDocument,
  dropped = 0,
): CoordinatorState {
  return {
    ...initialCoordinatorState(state.total),
    sessionId: state.sessionId,
    state: "imported",
    matches,
    capped: matches.length >= RESULT_CAP,
    query,
    importedDropped: dropped,
    target: {
      query: state.target?.query ?? query,
      matches: mergeMatches(state.target?.matches ?? [], matches).matches,
      sources: poolSources(state.target, matches, query),
    },
  };
}

function poolSources(
  pool: TargetState | undefined,
  matches: ParsedSeed[],
  query: QueryDocument,
): Record<number, QueryDocument> {
  const sources = { ...pool?.sources };
  for (const match of pool?.matches ?? []) sources[match.value] ??= pool!.query;
  for (const match of matches) sources[match.value] ??= query;
  return sources;
}

/** Retain every discovery, including partial results from failed runs. */
export function settleRun(state: CoordinatorState): CoordinatorState {
  if (!state.query) return state;
  return {
    ...state,
    target: {
      query: state.target?.query ?? state.query,
      matches: mergeMatches(state.target?.matches ?? [], state.matches).matches,
      sources: poolSources(state.target, state.matches, state.query),
    },
  };
}

const sumScanned = (workerScanned: Record<number, number[]>): number =>
  Object.values(workerScanned).reduce(
    (sum, scanned) => sum + scanned.reduce((s, value) => s + value, 0),
    0,
  );

export interface ProgressUpdate {
  sessionId: number;
  workerId: number;
  scanned: number[];
  matches: ParsedSeed[];
  now: number;
}

export function applyProgress(state: CoordinatorState, update: ProgressUpdate): CoordinatorState {
  // Progress is also accepted while stopping: the final flush carries matches
  // and counts from the region recorded as scanned, which a refine relies on.
  if (
    update.sessionId !== state.sessionId ||
    (state.state !== "running" && state.state !== "stopping" && state.state !== "failed")
  )
    return state;
  const workerScanned = { ...state.workerScanned, [update.workerId]: update.scanned };
  const tested = sumScanned(workerScanned);
  const merged = mergeMatches(state.matches, update.matches);
  const rateSamples = [...state.rateSamples, { at: update.now, tested }].filter(
    (sample) => update.now - sample.at <= 5_000,
  );
  // `capped` reports display truncation; the run itself ends on its own
  // accept quota, so a refine whose survivors already fill the display still
  // scans for more.
  const saturated = runSaturated({ ...state, matches: merged.matches });
  return settleRun({
    ...state,
    workerScanned,
    tested,
    matches: merged.matches,
    capped: merged.capped,
    state: saturated && state.state === "running" ? "stopping" : state.state,
    elapsed: update.now - state.startedAt,
    rateSamples,
    rate: calculateRate(rateSamples),
  });
}

export interface WorkerTerminal {
  sessionId: number;
  workerId: number;
  scanned: number[];
  kind: "done" | "stopped";
  now: number;
}

export function markWorkerDone(state: CoordinatorState, update: WorkerTerminal): CoordinatorState {
  if (
    update.sessionId !== state.sessionId ||
    (state.state !== "running" && state.state !== "stopping")
  )
    return state;
  const workerScanned = { ...state.workerScanned, [update.workerId]: update.scanned };
  const completedWorkers = state.completedWorkers + 1;
  const finished = completedWorkers >= state.workerCount;
  return settleRun({
    ...state,
    workerScanned,
    tested: sumScanned(workerScanned),
    completedWorkers,
    state: finished
      ? state.state === "stopping" && !runSaturated(state)
        ? "cancelled"
        : "completed"
      : state.state,
    elapsed: update.now - state.startedAt,
  });
}
