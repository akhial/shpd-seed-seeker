# Refinement ignored explicit trinket choices on native platforms

Date: 2026-09-21

## Impact

Refining saved AutoTrinket results could report matches whose loot required the
old automatic trinket, even after the current query specified another trinket.
The reported example added early armor and Wondrous Resin to a query whose saved
results used Mimic Tooth or Cracked Spyglass. Those seeds offered Resin, but
applying Resin changed their generated loot and invalidated other requirements.

Android and macOS were confirmed affected by the user. Windows and Linux use
the same affected native session function, so they had the same defect. Web
already rejected these false positives. Disabling AutoTrinket also exposed the
same missing override for a saved non-null choice.

This was a world-selection bug, not a seed-pool or 1,024-result-limit bug. A
trinket appearing among the initial offers does not imply that loot generated
with a different trinket exists when the requested trinket is applied.

## Why Web and native behavior drifted

The preservation work correctly retained a seed's original automatic choice
when ordinary item requirements changed. It also recovered choices previously
removed as unnecessary. However, the implementation over-applied preservation:
it did not make the current query's explicit world selection take precedence.

In `auto_trinkets::refine_batch`, a saved `Some(trinket)` was always reused.
The `enabled(query) && enabled(base)` check only controlled recovery from
`None`; it did not guard existing non-null recipes. The no-trinket cleanup then
returned immediately when automatic selection was disabled for the query.
Consequently, the old choice survived both generation and result serialization.

The item matcher could accept Wondrous Resin's presence in that wrong world.
Applying a selected trinket is the generation plan's responsibility, and the
saved recipe had overridden that plan. Scout subsequently applying Resin
exposed the discrepancy.

The adapters did not call the shared code under identical conditions:

| Platform | Refinement path before the fix |
| --- | --- |
| Web | The WASM adapter reused recipes only when `auto_trinkets::enabled(query)` was true. Otherwise it evaluated the seeds with the current query plan. |
| macOS, Windows | The C ABI called `production_filter_packet`, then `filter_matching_recipes`, then `refine_batch` without Web's eligibility check. |
| Android | JNI called the same native packet/session functions without that check. |
| Linux | Called `filter_matching_recipes` directly, reaching the same defective branch. |

Sharing the generator and matcher did not guarantee parity: the rule deciding
which world they evaluated existed only in the Web adapter. The earlier
refinement changes retained that Web-only guard while introducing native
recipe preservation. Review and tests missed the difference in entry conditions.

## Why existing tests missed it

Coverage exercised automatic-to-automatic edits, retention of required choices,
recovery of unnecessary choices, and explicit selection in fresh searches.
Those cases all worked. It did not combine a saved non-null automatic recipe
with a newly explicit trinket requirement, or with AutoTrinket being turned off,
through both platform adapters.

There was also a lower-level generation test that equated user refinement with
forced recipe replay for explicit-trinket queries. That expectation has been
corrected: forced replay still tests arbitrary worlds, while user refinement
must follow the current query's selection.

## Fix and prevention

`refine_batch` now checks the current query before using saved choices. Any
trinket requirement disables automatic selection, even if the AutoTrinket
toggle remains on. In that case, or when the toggle is off, seeds are evaluated
with the current query's explicitly selected trinket, or with no trinket when
none is selected.

When automatic selection remains eligible, original recipe preservation and
recovery of previously unnecessary choices continue unchanged. All saved seeds
remain in the pool; only the current matches change.

The Web path with a source query now invokes the same core decision as native
clients. Its legacy path without a source query retains the existing override
behavior and cannot reconstruct stripped historical choices. The lower-level
`filter_batch` remains a forced-replay primitive for generation parity tests;
its documentation now distinguishes it from user-facing refinement.

A shared fixture reconstructs the reported query and six reproducible false
positives from the screenshot. Before the fix, the native test returned all
six under their old trinkets; the Web test returned none. Both adapters must
now reject them for the explicit Resin query. Positive tests also ensure a
valid explicit Resin query applies Resin instead of rejecting every changed
choice, including when the saved recipe is null. Tests cover unselected trinket
requirements, AutoTrinket off, and Web's legacy path.

Additional Android, Windows, and macOS bridge regressions exercise the
automatic-to-explicit/off transitions through each application's engine API.
Linux is covered through its actual shared session entry point.

## Validation

- Rust core, session, WASM, C ABI, and JNI test command passed: 549 core unit
  tests, 82 core integration tests, 22 session tests, 18 WASM tests, and 12 C ABI
  tests. The suite's 13 existing ignored calibration/extended tests stayed
  ignored. JNI's Rust target has no unit tests; its real bridge is tested below.
- Android `AutoTrinketTest`: 3 tests passed against the rebuilt host JNI library.
- Windows `AutoTrinketTests` and `PreservedRefinementTests`: 4 tests passed
  against the rebuilt native DLL.
- Strict Clippy checks passed for all five Rust crates and all test targets.
- The WASM crate passed `cargo check --target wasm32-unknown-unknown`.
- Rust formatting and `git diff --check` passed.

The original imported 1,024-seed file was not available; reproduction uses the
known seeds and query reconstructed from the screenshots. The new Swift bridge
regression was added but cannot run on this Windows/WSL host. No macOS or Linux
GUI execution is claimed; their shared native paths are covered above.
