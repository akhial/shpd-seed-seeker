# Refinement and automatic trinkets — postmortem

## What happened

Adding an armor requirement to an imported 108-seed query could report
“Unrelated query” and start over. Other refinements only checked the imported
list and stopped, even with fewer than 1,024 matches. The same underlying
selection and start-decision rules affected all five graphical platforms.

## Causes

1. Automatic trinket ranking depends on the whole query. Adding armor could
   change that ranking, and the continuation predicate treated the change as
   a different generated world. That guard was correct for a freshly ranked
   search, but the application offered no way to preserve the original policy.
2. Eligibility to reuse scan coverage was also used to decide whether saved
   seeds could be checked at all. Shared-item relationships became filter-only;
   unrelated queries skipped the saved list.
3. Imports used an empty remaining range to represent missing scan history.
   Controllers interpreted it as a completed traversal, so there was no scan
   after filtering.
4. Native accept limits count delivered matches, including rediscovered saved
   seeds. A controller must count unique results across both phases and resume
   a capped native batch if the displayed list is still short.

Existing tests asserted several of these old behaviors, so passing them did
not protect the intended workflow.

## Changes

- Refinement keeps the original automatic selection rules through filtering,
  continued scanning, repeated refinements, and Android process recovery.
  Added requirements can change a fresh search's ranking without preventing
  refinement under the original ranking.
- A previously unnecessary trinket is reapplied before testing the refined
  query. Successful matches get the normal no-trinket cleanup. A failure in
  the selected world is accepted; there is no plain-world fallback to rescue
  it. This preserves the initial search's accepted tradeoff.
- Search checks saved results, then scans toward 1,024 unique results. Imports
  have explicitly unknown coverage and begin a fresh traversal after filtering.
  Known exhausted ranges remain exhausted. A full list can still request
  another batch, preserving the existing accumulating-search behavior.
- Every graphical platform has **Filter loaded seeds** for an intentional
  filter without scanning, including unrelated queries. Filtering B over a
  saved A list finds their intersection within that list. The full original
  list remains available for subsequent filters.

The engine separates ordinary freshly ranked continuation from containment
under a preserved selection policy. Native and browser execution use the same
prepared policy and authoritative matcher. Frontends retain the source query
separately from the current query, rather than changing the user's editor.

## Validation and limits

Regression tests cover the armor/ranking change, reapplying a removed trinket,
accepted misses without a plain-world retry, native/browser execution parity,
imported scans, unrelated filtering, duplicate results at the limit, repeated
refinements, and Android checkpoint recovery.

- Shared Rust engine and bridges: 704 tests passed; Clippy passed.
- Web: 286 tests passed; type checking, lint, and production build passed.
- Windows: 262 tests passed; the full WinUI build passed.
- Android: 42 targeted tests passed; debug lint passed.
- Linux: 126 tests passed; Clippy passed in Fedora 44 with the required GTK
  and libadwaita versions.
- macOS: implementation and regression tests updated, but not run because
  this environment has no macOS runner.

The Rust and Linux suites retained 13 and 3 pre-existing ignored tests,
respectively. No Android APK was built as part of this change.

The result-file format has not changed. Non-null trinket recipes replay exactly.
The original selection query is retained within a session (and Android's
private checkpoint), but is not included in exported files. After exporting a
refined query and importing it into a new session, a null recipe uses the
exported query's policy when automatic selection is reapplied.

Search remains heuristic: preserving automatic choices deliberately accepts
their misses. Cancellation, impossible queries, and exhausted coverage can
stop below the display limit. As before, mixed refinements do not claim an
exhaustive enumeration of every earlier, broader query.
