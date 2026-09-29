import { getItem, wildcardSpriteForKind, wildcardSprites } from "../game/catalog";
import type { RequirementKind } from "../../engine/types";
import { ARCANE_RESIN_SPRITE, itemArt } from "./sprites";
import type { ItemArt } from "./sprites";

/**
 * The art for a requirement's chip or sheet: its item, else its kind's
 * wildcard. A requirement describes what to search for rather than what some
 * run holds, so it is resolved without a gem table and a ring keeps the
 * catalog's per-class cell — the same picture whatever seed is on screen.
 */
export function requirementArt({
  item,
  kind,
}: {
  item?: string | null;
  kind?: RequirementKind | null;
}): ItemArt {
  if (item === "arcane_resin") return itemArt(ARCANE_RESIN_SPRITE);
  const named = item ? getItem(item) : undefined;
  if (named) return itemArt(named.sprite);
  return itemArt(kind ? wildcardSpriteForKind(kind) : wildcardSprites.weapon);
}
