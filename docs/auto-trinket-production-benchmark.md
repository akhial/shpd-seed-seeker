# Production auto-trinket benchmark

These recorded measurements predate the match-only no-trinket recheck. Current
search keeps the same initial match set, then strips unnecessary choices by
replaying only auto-applied matches. The runnable adapter now counts those
extra generations and checks returned recipes; the historical timing data
below does not measure that added cost.

This validates the implemented `auto_trinkets::search_batch` path, including its
prepared `QueryPlan`, choice resolution, pruning, matching, and result recipes.
The earlier [single-trinket study](single-trinket-study.md) measured a prototype;
its results are separate. In this production policy, Mossy Clump and Trap
Mechanism are excluded even from fallback choices.

## Results

The production policy is useful for these enchantment and ring searches without
the earlier branching cost. Each row searched **32,768 seeds per mode**. Arrows
show baseline → automatic selection.

| Query | Matches | Seeds/second | Native seconds/match | Less time/match (95% interval) |
|---|---:|---:|---:|---:|
| Exact +1 Grim Runic Blade, floors 1–19 | 33 → 67 | 257.4 → 256.8 | 3.857 → 1.904 | **50.6%** (38.7–63.1%) |
| Exact +2 Ring of Might, floors 1–9 | 355 → 404 | 667.2 → 666.7 | 0.1383 → 0.1217 | **12.1%** (8.0–16.1%) |
| Exact +1 Annoying melee weapon, floors 1–9 | 1,832 → 1,859 | 664.5 → 665.8 | 0.02692 → 0.02648 | **1.6%** (0.3–3.0%) |

Observed generation cost per seed changed by +0.24%, +0.09%, and −0.19%,
respectively. Preparing the automatic query plan took 0.39–0.96 milliseconds;
baseline preparation took 0.003–0.010 milliseconds. These are native measurements,
and the small throughput differences should not be interpreted as precise
predictions for another machine. Process CPU-time savings per match were 50.7%,
12.3%, and 1.8%, close to the wall-time results.

The curse control shows a small improvement from Mimic Tooth; Parchment Scrap
was never selected. Its practical gain is much smaller than the Grim Blade gain.
This supports a query-aware optional feature, not a claim that every search
becomes substantially faster. Different samples explain why this study's point
estimates differ from the historical prototype study; the new implementation
was not benchmarked against that prototype on this sample.

Automatic selection found 35 new Grim Blade matches while losing 1 baseline
match; Ring of Might gained 68 and lost 19; the curse control gained 56 and lost
29. Those losses are expected when choosing one world per seed and are why the
saved trinket recipe matters. All **4,550 returned matching recipes** reproduced
their query in independent full floor-24 scouts. All 98,304 automatic choices
were valid initial offers, with zero forbidden selections. The search API
received exactly 196,608 generation inputs across both modes and all queries.

Cracked Spyglass was selected on 6,206 seeds in the Ring query: that subset had
69 baseline matches and 72 automatic matches, with 3 gained and none lost.
This is a small supporting observation, not a precise independent estimate of
Spyglass's benefit. These three workloads do not establish a separate benefit
for every eligible trinket.

[Machine-readable summary](benchmarks/auto-trinket-production/summary.json)
includes timing, throughput, plan setup, intervals, and validation counts.

## Method

- Three queries, each with a fixed sample of 32,768 seeds per mode: exact +1 Grim
  Runic Blade through floor 19, exact +2 Ring of Might through floor 9, and exact
  +1 Annoying melee weapon through floor 9 as a curse control.
- Baseline uses `auto_apply_trinket: false`; automatic uses the same query with
  that flag set to `true`. Each input invokes world generation once per mode.
  Automatic selection uses one of the four initial offers at the existing +3
  brewing opportunity. No challenges are enabled.
- Both modes use the same dispersed seeds, with indices 300,000–332,767 in the
  recorded seed formula. These indices were not used by the earlier study.
- Native release build, one benchmark process on an 8-vCPU shared Linux host.
  Alternate AB/BA mode order in blocks of 32 seeds. Both modes are warmed up.
  Query-plan setup is measured separately. Timings include result matching and
  world destruction; seed-list creation and validation are outside the timers.
  There is no early stop after a match. The complete run took about eight minutes,
  including untimed replays. A single-worker browser smoke search overlapped
  approximately 30 seconds near the end of the Grim Blade run; no builds or broad
  test suites overlapped. The paired ordering and agreement with process CPU time
  reduce concern that the measured benefit came from this temporary host load.
- Resample adjacent AB/BA pairs 5,000 times for percentile bootstrap 95% intervals
  on the change in measured time per match. This preserves paired match outcomes
  and timing order; intervals describe sampling uncertainty in these workloads.
- Every returned match is independently replayed as a full floor-24 scout using
  its returned recipe. Every automatic choice is checked against its initial
  offers. Assertions reject Mossy Clump, Trap Mechanism, and, for the curse query,
  Parchment Scrap.

This measures native throughput and total generation seconds divided by matches.
It does **not** measure browser time to the first displayed result: WASM speed,
worker scheduling, 256-seed delivery batches, and startup affect that latency.
Results are query dependent, and choosing a different world can lose matches
that baseline would find.

## Reproduction

```sh
cargo build --locked --release -p shpd-seedfinder-core --features json-query --example auto_trinket_production_benchmark
python3 tooling/benchmarks/auto_trinket_production.py run
python3 tooling/benchmarks/auto_trinket_production.py summarize
```

[Environment and source hashes](benchmarks/auto-trinket-production/environment.json)
identify the measured working-tree implementation and release binary. Raw paired
blocks and replay counts are retained in
[the benchmark directory](benchmarks/auto-trinket-production/).
