import { readFile } from "node:fs/promises";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, describe, expect, it, vi } from "vite-plus/test";
import { QueryPanel } from "../designs/one/QueryPanel";
import { probabilityLabel } from "./format";
import {
  defaultQueryState,
  fromQueryJson,
  toQueryDocument,
  toQueryJson,
  validateQuery,
} from "./query";
import { decodeResultsFile, encodeResultsFile } from "./results-file";
import { loadPresets, queryStore, savePresets } from "./store";
import init, {
  SearchSession,
  analyze_query,
  decide_start,
  decode_share_text,
  encode_share_link,
  filter_seeds,
  scout,
} from "./wasm/pkg/seedfinder.js";
import type { ScoutResult, SearchAdvance } from "./wasm/types";

beforeAll(async () => {
  await init({
    module_or_path: await readFile(new URL("./wasm/pkg/seedfinder_bg.wasm", import.meta.url)),
  });
});

afterEach(() => {
  queryStore.setState(defaultQueryState);
  vi.unstubAllGlobals();
});

const document = {
  arcane_resin: 3,
  requirements: [{ item: "wand_lightning", upgrade: 2 }],
};

describe("Arcane Resin", () => {
  it("analyzes wider Auto wand queries quickly, including AutoTrinket", () => {
    for (const count of [4, 8]) {
      const query = {
        arcane_resin: "auto",
        auto_apply_trinket: true,
        requirements: Array.from({ length: count }, () => ({ kind: "wand" })),
      };
      const started = performance.now();
      const analysis = JSON.parse(analyze_query(JSON.stringify(query)));
      const elapsed = performance.now() - started;
      expect(analysis).toMatchObject({ valid: true, impossible: false });
      expect(analysis.probability).toBeGreaterThanOrEqual(0);
      expect(analysis.probability).toBeLessThanOrEqual(1);
      // Allows CI contention while catching the original multi-second stalls.
      expect(elapsed, `${count} Auto wands took ${elapsed.toFixed(1)} ms`).toBeLessThan(1000);
      if (count === 4) expect(analysis.probability).toBeGreaterThan(0.1);
    }
  });

  it("preserves the minimum in editor state, presets, share links and results files", () => {
    const storage = new Map<string, string>();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => storage.get(key) ?? null,
      setItem: (key: string, value: string) => storage.set(key, value),
    });
    for (const arcane_resin of [3, "auto"] as const) {
      for (const [requirements, arcane_resin_filter] of [
        [[], undefined],
        [document.requirements, undefined],
        [[], { uncursed: false, max_depth: 4, source: "chest" }],
        [document.requirements, { max_depth: 9 }],
      ]) {
        const state = fromQueryJson(
          JSON.stringify({ ...document, arcane_resin, requirements, arcane_resin_filter }),
        );
        expect(validateQuery(state).valid).toBe(true);
        expect(fromQueryJson(toQueryJson(state))).toEqual(state);
        expect(fromQueryJson(decode_share_text(encode_share_link(toQueryJson(state))))).toEqual(
          state,
        );
        savePresets([{ name: "Resin", query: state }]);
        expect(loadPresets()).toEqual([{ name: "Resin", query: state }]);
        const exported = encodeResultsFile(toQueryDocument(state), ["AAA-AAA-AAS"]);
        expect(decodeResultsFile(exported).query).toEqual(state);
      }
    }
    expect(fromQueryJson('{"requirements":[]}').arcaneResin).toBeUndefined();
    expect(
      toQueryDocument({ ...defaultQueryState(), arcaneResin: 0 }).arcane_resin,
    ).toBeUndefined();
  });

  it("rejects malformed amounts and allows resin-only searches", () => {
    for (const value of [-1, 1.5, 65536, "6", "Auto", "automatic", {}, true, null]) {
      const json = JSON.stringify({ ...document, arcane_resin: value });
      expect(() => fromQueryJson(json)).toThrow(/Arcane Resin/);
      expect(JSON.parse(analyze_query(json)).valid).toBe(false);
    }
    for (const arcaneResin of [NaN, Infinity, -1, 0.5, 65536]) {
      expect(validateQuery({ ...defaultQueryState(), arcaneResin }).valid).toBe(false);
    }
    const json = JSON.stringify({ requirements: [], arcane_resin: 6 });
    expect(validateQuery(fromQueryJson(json)).valid).toBe(true);
    expect(JSON.parse(analyze_query(json))).toMatchObject({
      valid: true,
      impossible: false,
      probability: expect.any(Number),
    });
    expect(validateQuery({ ...defaultQueryState(), arcaneResin: 0 }).valid).toBe(false);
  });

  it("searches, scouts, and refines Auto through the real engine", () => {
    // This seed's required +2 Lightning needs 3 resin, matching the fixed query.
    const auto = { ...document, arcane_resin: "auto" };
    const json = JSON.stringify(auto);
    const fixed = JSON.stringify(document);
    const session = new SearchSession(json, 18, 19);
    try {
      const found = JSON.parse(session.advance(1)) as SearchAdvance;
      expect(found.matches.map((match) => match.code)).toEqual(["AAA-AAA-AAS"]);
      expect(JSON.parse(filter_seeds(json, new Float64Array([18])))).toEqual(found.matches);
      const manifest = JSON.parse(
        scout(JSON.stringify({ seed: "AAA-AAA-AAS", query: auto })),
      ) as ScoutResult;
      expect(manifest.matchedRequirements).toBe(2);
      expect(manifest.totalRequirements).toBe(2);
      expect(manifest.items.filter((item) => item.matched)).toEqual(
        (
          JSON.parse(scout(JSON.stringify({ seed: "AAA-AAA-AAS", query: document }))) as ScoutResult
        ).items.filter((item) => item.matched),
      );
      expect(JSON.parse(analyze_query(json))).toMatchObject({
        valid: true,
        impossible: false,
        probability: expect.any(Number),
      });
      expect(decide_start(json, fixed, false, true)).toBe("target-filter");
      expect(decide_start(fixed, json, false, true)).toBe("target-filter");
      expect(decide_start(json, json, false, true)).toBe("target-refine");
    } finally {
      session.free();
    }
  });

  it("reserves the +2 Lightning wand while searching, scouting and refining", () => {
    const json = JSON.stringify(document);
    const session = new SearchSession(json, 18, 19);
    try {
      const found = JSON.parse(session.advance(1)) as SearchAdvance;
      expect(found.matches.map((match) => match.code)).toEqual(["AAA-AAA-AAS"]);
      const manifest = JSON.parse(
        scout(JSON.stringify({ seed: "AAA-AAA-AAS", query: document })),
      ) as ScoutResult;
      expect(manifest.matchedRequirements).toBe(2);
      expect(manifest.totalRequirements).toBe(2);
      const marked = manifest.items.filter((item) => item.matched);
      const reserved = marked.find((item) => item.id === "wand_lightning" && item.upgrade === 2);
      expect(reserved).toBeDefined();
      const surplus = marked.filter((item) => item !== reserved);
      expect(surplus.every((item) => item.category === "wand" && !item.cursed)).toBe(true);
      expect(
        surplus.reduce((total, item) => total + 2 * (item.upgrade + 1), 0),
      ).toBeGreaterThanOrEqual(3);
      expect(JSON.parse(filter_seeds(json, new Float64Array([18])))).toEqual(found.matches);
      const harder = JSON.stringify({ ...document, arcane_resin: 100 });
      expect(JSON.parse(filter_seeds(harder, new Float64Array([18])))).toEqual([]);
      expect(decide_start(harder, json, false, true)).toBe("target-refine");
      expect(decide_start(json, harder, false, true)).toBe("target-filter");
    } finally {
      session.free();
    }
  });

  it("reports probability for resin-only and mixed queries through the real engine", () => {
    for (const requirements of [[], document.requirements]) {
      const json = JSON.stringify({ ...document, requirements });
      const state = fromQueryJson(json);
      queryStore.setState(() => state);
      const analysis = JSON.parse(analyze_query(json));
      expect(analysis).toMatchObject({ valid: true, impossible: false });
      expect(analysis.probability).toBeGreaterThan(0);
      expect(analysis.probability).toBeLessThanOrEqual(1);
      const html = renderToStaticMarkup(
        <QueryPanel
          analysis={analysis}
          validation={validateQuery(state)}
          running={false}
          engineReady
          isMac={false}
          onToggleSearch={() => {}}
          shareNotice={undefined}
          onDismissShareNotice={() => {}}
        />,
      );
      expect(html).toContain(probabilityLabel(analysis.probability));
      expect(html).not.toContain("Probability unavailable");
    }
    const baseline = JSON.parse(
      analyze_query(JSON.stringify({ ...document, arcane_resin: 0 })),
    ).probability;
    const constrained = JSON.parse(
      analyze_query(JSON.stringify({ ...document, arcane_resin: 12 })),
    ).probability;
    expect(constrained).toBeLessThan(baseline);
  });

  it("shows a resin requirement chip without the old control or helper text", () => {
    queryStore.setState(() => fromQueryJson('{"arcane_resin":6,"requirements":[]}'));
    const html = renderToStaticMarkup(
      <QueryPanel
        analysis={undefined}
        validation={validateQuery(queryStore.state)}
        running={false}
        engineReady
        isMac={false}
        onToggleSearch={() => {}}
        shareNotice={undefined}
        onDismissShareNotice={() => {}}
      />,
    );
    expect(html).toContain("Arcane Resin");
    expect(html).toContain('aria-label="Edit Arcane Resin"');
    expect(html).toContain('aria-label="Remove Arcane Resin"');
    expect(html).toContain("≥6");
    expect(html).toContain("1 requirement");
    expect(html).not.toContain("arcane-resin-help");
    expect(html).not.toContain("extra uncursed wands");
    expect(html).not.toContain("Your required wands are kept.");
  });

  it("keeps numeric zero estimates distinct from unavailable probabilities", () => {
    const analysis = JSON.parse(
      analyze_query(JSON.stringify({ ...document, arcane_resin: 65535, auto_apply_trinket: true })),
    );
    expect(analysis).toMatchObject({ valid: true, probability: 0 });
    expect(probabilityLabel(analysis.probability)).toBe("Match probability ≈ 0");
    expect(probabilityLabel(null)).toBe("Probability unavailable");
  });

  it("rejects malformed resin filters before searching", () => {
    for (const arcane_resin_filter of [
      null,
      { uncursed: "yes" },
      { max_depth: 0 },
      { max_depth: 25 },
      { source: "unknown" },
    ]) {
      const json = JSON.stringify({ ...document, arcane_resin_filter });
      expect(() => fromQueryJson(json)).toThrow(/Arcane Resin/);
      expect(JSON.parse(analyze_query(json)).valid).toBe(false);
    }
  });
});
