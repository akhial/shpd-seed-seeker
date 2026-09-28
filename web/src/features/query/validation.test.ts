import { describe, expect, it } from "vite-plus/test";
import { clampUpgrade, defaultQueryState, fromQueryJson, validateRequirement } from "./query";
import { validateQuery } from "./validation";
import type { RequirementState } from "../../engine/types";

const requirement = (patch: Partial<RequirementState> = {}): RequirementState => ({
  key: 1,
  kind: "weapon",
  tier: { mode: "any", value: 3 },
  upgrade: { mode: "any", value: 1 },
  uncursed: false,
  ...patch,
});

describe("query validation", () => {
  // The requirement rules and their wording belong to the shared core
  // (docs/requirement-editor.md), which tests them; these cases pin how the
  // query-level checks and the core's first problem meet.

  it("accepts a valid query", () => {
    const query = fromQueryJson(
      '{"requirements":[{"kind":"weapon","tier":{"at_least":3},"upgrade":{"at_least":2},"effect":"Blazing","identity_group":1},{"kind":"weapon","identity_group":1}],"max_depth":20,"require_blacksmith":true,"challenges":["on_diet"]}',
    );
    expect(validateQuery(query)).toEqual({ valid: true, errors: [] });
  });

  it("numbers a requirement's problem by the row's place in the list, not its key", () => {
    const query = {
      ...defaultQueryState(),
      requirements: [
        requirement({ key: 7, kind: "wand" }),
        requirement({
          key: 3,
          kind: "ring",
          item: "ring_haste",
          upgrade: { mode: "exact", value: 5 },
        }),
      ],
    };
    const { valid, errors } = validateQuery(query);
    expect(valid).toBe(false);
    expect(errors).toHaveLength(1);
    expect(errors[0]).toMatch(/^Requirement 2: /);
  });

  it("numbers a problem between rows by the first row it blames", () => {
    const query = fromQueryJson(
      '{"requirements":[{"kind":"wand"},{"item":"ring_might","level_sum":{"group":1,"at_least":2}},{"item":"ring_might","level_sum":{"group":1,"at_least":3}}]}',
    );
    expect(validateQuery(query).errors).toEqual([
      "Requirement 2: A stack must share one combined level.",
    ]);
  });

  it("says the list's own problem without a number", () => {
    const query = fromQueryJson('{"requirements":[{"kind":"wand","blanket":true}]}');
    expect(validateQuery(query).errors).toEqual(["Add at least one ordinary requirement."]);
  });

  it("checks the query's own settings before its requirements", () => {
    const query = {
      ...fromQueryJson('{"requirements":[{"kind":"wand","max_depth":30}]}'),
      maxDepth: 30,
    };
    expect(validateQuery(query).errors).toEqual([
      "Maximum floor must be 1 through 24.",
      "Requirement 1: Requirement floor must be 1 through 24.",
    ]);
    expect(validateQuery(defaultQueryState()).errors).toEqual(["Add at least one requirement."]);
  });

  it("pulls an out-of-reach upgrade back when the tier narrows", () => {
    const plus5 = requirement({ upgrade: { mode: "exact", value: 5 } });
    expect(clampUpgrade(plus5)).toBe(plus5);
    expect(clampUpgrade({ ...plus5, tier: { mode: "exact", value: 5 } }).upgrade).toEqual({
      mode: "exact",
      value: 4,
    });
    expect(clampUpgrade({ ...plus5, item: "sword" }).upgrade).toEqual({ mode: "exact", value: 4 });
    expect(clampUpgrade({ ...plus5, item: "battle_axe" }).upgrade).toEqual({
      mode: "exact",
      value: 5,
    });
    // An "at least" bound stops one below the ceiling: the top level is what
    // "exactly" already says.
    const atLeast4 = requirement({ upgrade: { mode: "at_least", value: 4 } });
    expect(clampUpgrade({ ...atLeast4, tier: { mode: "exact", value: 5 } }).upgrade).toEqual({
      mode: "at_least",
      value: 3,
    });
  });

  it("keeps the editor from saving an empty effect choice", () => {
    expect(validateRequirement(requirement({ effect: [] }))).toEqual([
      "Choose at least one effect.",
    ]);
  });
});
