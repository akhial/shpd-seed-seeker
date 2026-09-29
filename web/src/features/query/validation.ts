import { floorRequirementErrors } from "../../shared/game/floor-requirements";
import type { QueryState } from "../../engine/types";
import { MAX_DEPTH, validArcaneResin, validateArcaneResinFilter } from "./query";
import { requirementBoardOf } from "./requirements/board";

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

/**
 * Whether the query can be searched or shared: the query's own settings
 * first, then the first problem the requirement editor finds in its
 * requirements, numbered by the requirement it blames.
 */
export function validateQuery(state: QueryState): ValidationResult {
  const errors: string[] = floorRequirementErrors(state.floorRequirements ?? [], state.maxDepth);
  if (!state.requirements.length && !state.arcaneResin && !state.floorRequirements?.length)
    errors.push("Add at least one requirement.");
  if (state.arcaneResin !== undefined && !validArcaneResin(state.arcaneResin))
    errors.push("Arcane Resin must be Auto or a whole number from 0 through 65535.");
  if (state.arcaneResinFilter) errors.push(...validateArcaneResinFilter(state.arcaneResinFilter));
  if (state.maxDepth < 1 || state.maxDepth > MAX_DEPTH)
    errors.push(`Maximum floor must be 1 through ${MAX_DEPTH}.`);
  const board = requirementBoardOf(state);
  const problem = board.ok ? board.value.problems[0] : { message: board.error, keys: [] };
  if (problem) {
    const position = state.requirements.findIndex(
      (requirement) => requirement.key === problem.keys[0],
    );
    errors.push(
      position >= 0 ? `Requirement ${position + 1}: ${problem.message}` : problem.message,
    );
  }
  return { valid: errors.length === 0, errors };
}
