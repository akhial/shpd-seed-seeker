import type { ScoutItem } from "../../engine/types";

export function matchedScoutChoices(items: readonly ScoutItem[]): Map<number, number> {
  const choices = new Map<number, number>();
  for (const item of items) {
    if (item.matched && item.accessibility.type === "choice") {
      choices.set(item.accessibility.group, item.accessibility.option);
    }
  }
  return choices;
}

export function isAlternateScoutChoice(
  item: ScoutItem,
  choices: ReadonlyMap<number, number>,
): boolean {
  return (
    !item.matched &&
    item.accessibility.type === "choice" &&
    choices.has(item.accessibility.group) &&
    choices.get(item.accessibility.group) !== item.accessibility.option
  );
}

/**
 * Whether Scout draws `item` dimmed: an alternate choice the match did not
 * take, or an item the query excludes outright (Smith rewards under
 * "Exclude smith rewards"), which is never matched.
 */
export function isDimmedScoutItem(item: ScoutItem, choices: ReadonlyMap<number, number>): boolean {
  return item.excluded === true || isAlternateScoutChoice(item, choices);
}

/** Scout items are fixed dungeon loot; runtime drops and transmutation outcomes are absent. */
export function availableArtifactIds(
  items: readonly ScoutItem[],
  choices = matchedScoutChoices(items),
): Set<string> {
  return new Set(
    items
      .filter((item) => item.category === "artifact" && !isAlternateScoutChoice(item, choices))
      .map((item) => item.id),
  );
}
