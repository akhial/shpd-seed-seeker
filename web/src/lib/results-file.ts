import packageJson from "../../package.json";
import { fromQueryJson } from "./query";
import { decodeResultsFileText, encodeResultsFileText, parseSeedCodeSync } from "./wasm";
import type { ParsedSeed, QueryDocument, QueryState } from "./wasm/types";

// The results-export document shared by every Seed Seeker frontend. The codec
// itself lives in the Rust core (crates/seedfinder-core/src/results_export.rs)
// and is reached through `encode_results_file`/`decode_results_file`; this
// module only maps between it and the browser's state.

export const RESULTS_FILE_NAME = "seed-seeker-results.json";

/** Canonical `ParsedSeed` for one imported seed code, valued by the engine. */
export function parsedSeedFromCode(code: string): ParsedSeed {
  return parseSeedCodeSync(code);
}

/**
 * Encodes the query document that produced `seeds` (a search-time snapshot).
 * The engine writes the envelope, including the game version it targets.
 *
 * @throws Error with the codec's message for an invalid query or seed code.
 */
export function encodeResultsFile(query: QueryDocument, seeds: string[]): string {
  const { auto_apply_trinkets, ...portableQuery } = query;
  const encoded = encodeResultsFileText(
    JSON.stringify({ query: portableQuery, seeds, app_version: packageJson.version }),
  );
  // Keep the web-only setting outside the portable query, so native importers can still read it.
  if (!auto_apply_trinkets) return encoded;
  return JSON.stringify({ ...JSON.parse(encoded), auto_apply_trinkets: true }, null, 2);
}

export interface DecodedResultsFile {
  appVersion?: string;
  shpdVersion?: string;
  /** The canonical query document the engine decoded, for re-serialization. */
  queryDocument: QueryDocument;
  /** The query decoded into editor state. */
  query: QueryState;
  /** Canonical seed codes, already deduplicated and capped by the engine. */
  seeds: string[];
  /** Exported entries the engine's dedupe-and-cap removed. */
  dropped: number;
}

interface DecodedDocument {
  query: QueryDocument;
  seeds: string[];
  dropped: number;
  app_version: string | null;
  shpd_version: string | null;
}

/**
 * Decodes a results-export document through the engine codec: the size limit,
 * the envelope rules (unknown fields from future releases are ignored), the
 * query validation, the seed-code form, and dedupe-and-cap are all its.
 *
 * @throws Error with the codec's user-facing message for unusable files.
 */
export function decodeResultsFile(text: string): DecodedResultsFile {
  const decoded = JSON.parse(decodeResultsFileText(text)) as DecodedDocument;
  if ((JSON.parse(text) as { auto_apply_trinkets?: boolean }).auto_apply_trinkets === true)
    decoded.query.auto_apply_trinkets = true;
  return {
    appVersion: decoded.app_version ?? undefined,
    shpdVersion: decoded.shpd_version ?? undefined,
    queryDocument: decoded.query,
    query: fromQueryJson(JSON.stringify(decoded.query)),
    seeds: decoded.seeds,
    dropped: decoded.dropped,
  };
}
