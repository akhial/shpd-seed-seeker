# Refinement postmortem

## Symptoms and causes

Adding armor to an imported query could trigger an “Unrelated query” message.
Automatic trinket ranking depends on all requirements, and the old continuation
check treated a changed ranking as an incompatible search. The same shared
rules affected all five graphical platforms.

Saved-seed verification was coupled to that coverage check. Some queries only
filtered, while others bypassed the saved seeds. Imported files had no scan
history, represented as an empty remaining range; that was mistaken for an
exhausted traversal. Native result caps also counted rediscovered saved seeds,
allowing searches to stop below 1,024 unique matches. The web coordinator also
marked a capped search complete before every worker flushed its final discoveries;
it now drains those reports into the pool before completing.

An initial fix preserved selection during resumed scans and added a separate
filter action. That left unnecessary choices in the UI and separate collections
for different query relationships. It did not match the requested simple model.

## Final behavior

All platforms now have one Search action. It checks the entire retained pool,
shows current matches, then searches for more. Every loaded or discovered seed
joins the pool, including imports and partial results from cancelled or failed
scans. Only Clear results discards it. Hidden seeds remain available to all
future queries.

The related/unrelated classification, containment proofs, routing modes, bridge
APIs, separate filter buttons, and special scan-selection envelope are removed.
Only an unchanged query resumes its previous cursor; query edits scan afresh.
This trades coverage reuse across edits for predictable behavior without
relationship inference.

Each saved seed retains its original recipe and source query. A removed
automatic trinket is reapplied before testing, without predicting whether it
will help. Misses in that world are accepted. Successful matches get the usual
no-trinket cleanup. New scans use the current query and record it as the source
of newly discovered seeds.

## Verification and remaining limits

Regression coverage checks the full pool across different queries and imports,
recovery of hidden matches, retention after failure/cancellation, original
trinket sources, unchanged-query resume, duplicate result quotas, and Android
checkpoint recovery. Tests of the removed relationship APIs were replaced by
these behavioral checks. Validation:

- Android: all 259 JVM tests and debug lint passed, including real host JNI tests.
- Web: all 265 tests, type/lint checks, and the production build passed with rebuilt WASM.
- Windows: all 241 tests and the application build passed.
- Linux: all 124 enabled tests and strict Clippy checks passed (three tests remain ignored).
- Shared Rust core, session, FFI, and WASM tests passed; strict Clippy checks also
  cover the JNI bridge.

macOS needs a macOS runner for its application build and tests. No Android APK
was built. The results-file schema is unchanged: exported matches retain their
recipes, but private source queries are not exported. A null recipe reimported
later uses the exported query's automatic selection policy.
