import { requirementBoard } from "../../../engine/editor";
import type { EditorAnswer } from "../../../engine/editor";
import type {
  BoardEdit,
  BoardRefusal,
  BoardResponse,
  QueryState,
  RequirementState,
  ResinCondition,
} from "../../../engine/types";
import { requirementFromRow, requirementToRow, validArcaneResin } from "../query";

/**
 * The requirement board as the shared core draws it (docs/requirement-editor.md):
 * every rule behind the chips, clusters and stacks, every word on them, and
 * every edit a gesture makes live in the core. This module only moves the
 * query's requirements in and out of the `requirement_board` envelope.
 */

/** The part of the query the board draws. */
export type BoardQuery = Pick<QueryState, "requirements" | "arcaneResin" | "arcaneResinFilter">;

/**
 * One past every key the editor has answered with. Sent with each request so
 * a new row never takes the key of one removed earlier, which board state
 * (an open stepper, a pick) may still hold.
 */
let nextKey = 1;

/** The query's Arcane Resin condition, which the board draws as its resin chip. */
function resinCondition(query: BoardQuery): ResinCondition | null {
  const amount = query.arcaneResin;
  if (!amount || !validArcaneResin(amount)) return null;
  const filter = query.arcaneResinFilter;
  return {
    amount,
    filter: filter
      ? {
          uncursed: filter.uncursed,
          max_depth: filter.maxDepth ?? null,
          source: filter.source ?? null,
          include_mage_wand: filter.includeMageWand ?? false,
        }
      : null,
  };
}

function ask(query: BoardQuery, edits: BoardEdit[] = []): EditorAnswer<BoardResponse> {
  const answer = requirementBoard({
    rows: query.requirements.map(requirementToRow),
    next_key: nextKey,
    edits,
    resin: resinCondition(query),
  });
  if (answer.ok) nextKey = Math.max(nextKey, answer.value.next_key);
  return answer;
}

let drawn: { query: BoardQuery; answer: EditorAnswer<BoardResponse> } | undefined;

const sameBoard = (left: BoardQuery, right: BoardQuery): boolean =>
  left.requirements === right.requirements &&
  left.arcaneResin === right.arcaneResin &&
  left.arcaneResinFilter === right.arcaneResinFilter;

const boardFields = ({ requirements, arcaneResin, arcaneResinFilter }: BoardQuery): BoardQuery => ({
  requirements,
  arcaneResin,
  arcaneResinFilter,
});

/**
 * The board of a query. The envelope runs once per change of the
 * requirements or the resin, however many readers (both board sections, the
 * header counts, the Start and Share gate) ask for it in between.
 */
export function requirementBoardOf(query: BoardQuery): EditorAnswer<BoardResponse> {
  if (drawn && sameBoard(drawn.query, query)) return drawn.answer;
  const answer = ask(query);
  drawn = { query: boardFields(query), answer };
  return answer;
}

/** What a board edit did. */
export interface BoardOutcome {
  /** The requirements after the edit: the query's own list when nothing changed. */
  requirements: RequirementState[];
  changed: boolean;
  /** Why the edit was refused, to say to the user. */
  refused: BoardRefusal | null;
  /** Keys the core renumbered, `[old, new]`, for state that holds keys. */
  rekeyed: [number, number][];
}

/**
 * Applies board edits in order. The rows are adopted only when the edits
 * changed something, so a no-op keeps the query's identity: an unchanged
 * query resumes a cancelled search and still matches its preset.
 */
export function editBoard(query: BoardQuery, edits: BoardEdit[]): EditorAnswer<BoardOutcome> {
  const answer = ask(query, edits);
  if (!answer.ok) return answer;
  const { changed, refused, rekeyed, rows } = answer.value;
  if (!changed)
    return { ok: true, value: { requirements: query.requirements, changed, refused, rekeyed } };
  let requirements: RequirementState[];
  try {
    requirements = rows.map(requirementFromRow);
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
  // The answer already draws the edited rows, so the next render reuses it.
  drawn = { query: { ...boardFields(query), requirements }, answer };
  return { ok: true, value: { requirements, changed, refused, rekeyed } };
}

/** A key as the core renumbered it, or the key itself. */
export const rekey = (key: number, rekeyed: readonly [number, number][]): number =>
  rekeyed.find(([old]) => old === key)?.[1] ?? key;
