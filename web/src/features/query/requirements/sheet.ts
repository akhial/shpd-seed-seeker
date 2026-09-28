import { requirementEditor } from "../../../engine/editor";
import type { EditorAnswer } from "../../../engine/editor";
import type {
  EditorChange,
  EditorResponse,
  EditorSheet,
  QueryState,
  RequirementState,
} from "../../../engine/types";
import { requirementFromRow, requirementToRow, resinCondition, resinFromCondition } from "../query";
import { nextKeyHint, noteNextKey } from "./board";
import type { BoardQuery } from "./board";

/**
 * The requirement sheet as the shared core keeps it (docs/requirement-editor.md):
 * what a control offers, what moving it resets, what a save writes and why it
 * may not all live in the core. The sheet is an opaque draft plus the form it
 * shows; this module only moves the query in and out of the
 * `requirement_editor` envelope.
 */

/** What a sheet opens on. */
export type SheetTarget =
  | { type: "row"; key: number }
  | { type: "new"; blanket: boolean }
  | { type: "resin" };

/** An open or change answer, which is always a sheet. */
function sheetOf(answer: EditorAnswer<EditorResponse>): EditorAnswer<EditorSheet> {
  if (!answer.ok) return answer;
  if ("saved" in answer.value) return { ok: false, error: "The sheet answered with a save." };
  return { ok: true, value: answer.value };
}

/** Opens a sheet on a chip, a new chip of either section, or the resin chip. */
export function openSheet(query: BoardQuery, target: SheetTarget): EditorAnswer<EditorSheet> {
  return sheetOf(
    requirementEditor({
      op: "open",
      rows: query.requirements.map(requirementToRow),
      key: target.type === "row" ? target.key : null,
      blanket: target.type === "new" && target.blanket,
      resin: resinCondition(query),
      // Arcane Resin is the query's own condition, which the sheet can set.
      offer_resin: true,
      open_resin: target.type === "resin",
    }),
  );
}

/** Applies one control the user moved. */
export function changeSheet(sheet: EditorSheet, change: EditorChange): EditorAnswer<EditorSheet> {
  return sheetOf(requirementEditor({ op: "change", draft: sheet.draft, change }));
}

/** What a save did to the query. */
export interface SheetSaved {
  /** The requirements after the save: the query's own list when nothing changed. */
  requirements: RequirementState[];
  changed: boolean;
  /** The query's resin when the save set or cleared it; absent when it stays as it was. */
  resin?: Pick<QueryState, "arcaneResin" | "arcaneResinFilter">;
  /** The chip the save landed on. */
  focus: number | null;
}

const sameResin = (
  left: Pick<QueryState, "arcaneResin" | "arcaneResinFilter">,
  right: Pick<QueryState, "arcaneResin" | "arcaneResinFilter">,
): boolean => JSON.stringify(resinCondition(left)) === JSON.stringify(resinCondition(right));

/**
 * Saves the sheet onto the query's requirements as they are now. A refused
 * save answers the sheet again, its reasons in `form.errors`. The rows are
 * adopted only when the save changed them, and the resin only when it moved,
 * so saving a chip unchanged keeps the query as it was.
 */
export function saveSheet(
  query: BoardQuery,
  sheet: EditorSheet,
): EditorAnswer<{ saved: SheetSaved } | { refused: EditorSheet }> {
  const answer = requirementEditor({
    op: "save",
    draft: sheet.draft,
    rows: query.requirements.map(requirementToRow),
    next_key: nextKeyHint(),
  });
  if (!answer.ok) return answer;
  if ("draft" in answer.value) return { ok: true, value: { refused: answer.value } };
  const { rows, changed, focus, next_key, resin } = answer.value.saved;
  noteNextKey(next_key);
  let requirements = query.requirements;
  try {
    if (changed) requirements = rows.map(requirementFromRow);
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
  const nextResin =
    resin === null
      ? undefined
      : "set" in resin
        ? resinFromCondition(resin.set)
        : { arcaneResin: undefined, arcaneResinFilter: undefined };
  return {
    ok: true,
    value: {
      saved: {
        requirements,
        changed,
        focus,
        ...(nextResin && !sameResin(nextResin, query) ? { resin: nextResin } : {}),
      },
    },
  };
}
