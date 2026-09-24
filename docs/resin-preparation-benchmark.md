# Resin and blanket preparation

The reported Android query spent several seconds preparing AutoTrinket's five
equipment profiles even after #172 removed UI blocking and repeated estimation.
The resin estimator rebuilt the same draw transitions for successive supply
counts, and recomputed reward-choice probabilities for states differing only
in resin balance or the costs of already reserved items.

Preparation now reuses those transitions within each supply stream. Choice
probabilities are keyed by which reservation groups still need an item. The
original event order, multiplication order, state order, pruning thresholds,
and 4,096-state work limit are retained. Transition history is bounded and
discarded at the end of the stream; it does not become a process-wide cache.

## Measurements

Native release builds on an Apple M4 Pro, macOS 27.0, Rust 1.98.0. Baseline is
main `8eeeb63c` (merged #172). Both builds use the same benchmark example and
release settings (opt-level 3, fat LTO, one codegen unit).

Each row has five samples per build, with alternating before/after order and a
fresh process per sample. No builds or test suites overlapped the measurements.
Times include `QueryPlan::analyze` and the final probability estimate, including
on-demand table loading. Medians and observed ranges are shown, not confidence
intervals or Galaxy S10 measurements.

| Query | Before | After | Speedup |
|---|---:|---:|---:|
| Reported mixed query, Auto Resin and early wand blanket | 2,911 ms (2,848–2,936) | 655 ms (652–660) | 4.4× |
| Same query without the blanket | 340 ms (338–346) | 74 ms (74–77) | 4.6× |
| Four arbitrary wands, Auto Resin, AutoTrinket, +1 wand blanket by floor 4 | 35.0 ms (34.7–35.3) | 26.4 ms (26.3–27.3) | 1.3× |

For the reported query, maximum resident memory measured with `/usr/bin/time -l`
was 7.20 MiB before and 10.27 MiB after (maximum of five samples). This is a
standalone native benchmark, not the Android app's total memory. The retained
transition limit is 131,072 edges; excess rows are calculated without retaining
them. Historical states are compacted when they exceed 8,192 between draws.

## Result preservation

Before/after comparison of 69 queries produced bit-for-bit identical final
probabilities, identical trinket rankings and identical feasibility results.
The cases covered the reported query and 16 edits, 48 combinations of 1/2/4/8
wands with Auto/4/12 resin and none/Auto/Mimic Tooth/Cracked Spyglass profiles,
and four allocation cases involving overlapping blankets, linked identities,
alternatives and excluded reservations. These comparisons generate no worlds.

Twelve representative cases are retained as public probability/ranking
regressions in `tests/fixtures/resin-preparation.json`, including larger queries
with distinct blanket witnesses and six reserved wands. The full Rust workspace
suite passed (790 tests; 13 existing opt-in tests ignored), excluding the GTK
application. Workspace Clippy and formatting checks passed.

## Reproduce

```sh
cargo build --locked --release -p shpd-seedfinder-core --features json-query --example benchmark_preparation
target/release/examples/benchmark_preparation 'https://shpd-seed-seeker.web.app/#q=qgAAAXLcAAlsAAQAAXPGABc8AAueAAfrAAP3AAEEQRi7AWCgHZCwAA'
target/release/examples/benchmark_preparation --queries crates/seedfinder-core/tests/fixtures/resin-preparation.json
```

The example also accepts a JSON query as its first argument. For baseline
measurements, copy the example into a checkout of `8eeeb63c` and build there.
Run a separate process for each cold timing sample; the multi-query mode is
for comparisons and uses a fresh intermediate-cache thread per case while
retaining the engine's process-wide query caches. Output includes probability
bits, trinket ranking, cold preparation time and cached preparation time.
