> Historical prototype study. The shipped policy and its independent validation
> are documented in [automatic selection](auto-apply-trinkets.md). The prototype
> below occasionally used Mossy Clump or Trap Mechanism as a last-resort choice;
> production excludes both completely.

**Single-choice trinket search study — 10 September 2026**

This study evaluates choosing one initial trinket before generation. It adds a reproducible research harness; production search and the web UI are unchanged. Results below use the current `4.0.0-RC-1` engine at `ba22483`.

**Recommendation: worth implementing as a query-aware option, especially for enchantment and glyph searches.** Choosing one trinket removes the previous branching cost. It does not improve every query, and web delivery batching limits first-result gains on easy searches.

The ten queries below tested **221,184 query/seed pairs per mode**, with **18.0 minutes of measured engine time** across both modes. Every selection run generated only its chosen world for each seed. Selection changed generation cost per seed by less than 1% in every case; ranking took 1.8–5.9 ms once per query.

The strongest named-item result is **+1 Grim Runic Blade through floor 19**: **38 → 68 matches** across 32,768 seeds, **3.388 → 1.904 seconds per match**, or **43.8% less engine time** (approximate 95% interval **30.1–57.0%**). Including the one-time ranking cost barely changes this result. The 256-seed delivery model also improves: **3.901 → 2.398 seconds to a nonempty batch**, or **38.5% less time** (interval **21.4–54.6%**). This is meaningful evidence for a first-result benefit on a rare named enchantment query.

All upgrade levels below are **exact**, with no challenges and no requirement that the equipment be uncursed. Positive percentages mean less time per match. Intervals that include zero do not establish a speed improvement; the mixed query has no baseline hits, so no finite comparative estimate is reported.

| Query | Through floor | Seeds per mode | Matches baseline → selected | Seconds/match baseline → selected | Time saved (approx. 95% interval) |
| --- | ---: | ---: | ---: | ---: | ---: |
| +1 Grim Runic Blade | 19 | 32,768 | 38 → 68 | 3.3877 → 1.9044 | 43.8% (30.1 to 57.0%) |
| +1 Thorns armor | 9 | 32,768 | 279 → 489 | 0.1774 → 0.1013 | 42.9% (38.6 to 47.4%) |
| +1 Grim melee weapon (any identity) | 19 | 16,384 | 432 → 686 | 0.1479 → 0.0939 | 36.5% (32.9 to 40.4%) |
| +2 Ring of Might | 9 | 32,768 | 347 → 400 | 0.1420 → 0.1234 | 13.1% (8.5 to 17.8%) |
| Ethereal Chains | 9 | 32,768 | 4,614 → 4,748 | 0.0108 → 0.0105 | 2.6% (1.8 to 3.5%) |
| +1 Annoying melee weapon | 9 | 16,384 | 890 → 904 | 0.0278 → 0.0276 | 0.8% (-1.0 to 2.4%) |
| +3 Grim Runic Blade | 19 | 32,768 | 19 → 22 | 6.7403 → 5.8365 | 13.4% (-0.3 to 29.8%) |
| +3 Plate Armor | 19 | 4,096 | 3,206 → 3,206 | 0.0050 → 0.0050 | 0.6% (-0.7 to 1.9%) |
| +3 Wand of Fireblast | 9 | 4,096 | 51 → 51 | 0.1099 → 0.1104 | -0.5% (-3.1 to 2.1%) |
| +1 Grim melee weapon AND +2 Ring of Might | 9 | 16,384 | 0 → 2 | — → 12.4101 | Too sparse |

The requested **+3 Grim Runic Blade** example remains **inconclusive**: 19 → 22 matches after 4.3 minutes of paired measurement. All three additional matches use Parchment Scrap, but a −0.3% to 29.8% interval is too broad to promise a particular speed gain. Existing source constraints limit its potential: +3 equipment comes from restricted sources, and some late rewards already guarantee good effects.

Parchment Scrap supplies **29 of the 30 net additional named +1 Grim blade matches**, **235 of the 254 net additional broad Grim weapon matches**, and **183 of the 210 net additional Thorns armor matches**. Mimic Tooth supplies **50 of the 53 net additional Ring of Might matches**. Those are the clearest initial choices supported by this study. The artifact query gains only 2.6% per match, the curse query has no clear gain, and the plain +3 controls retain identical match sets. The combined rare query's 0 → 2 results demonstrate additional recipes, not a measurable comparative speedup.

This policy also **loses some baseline matches**. The named +1 blade gains 33 seeds and loses 3; the ring gains 73 and loses 20. Its net match rate improves, but it does not preserve the baseline result set. One replayed example is `SRU-YSU-QHS`: choosing **Parchment Scrap +3** produces a **+1 Grim Runic Blade on floor 11, from a heap**; the baseline does not satisfy the query. This recipe was regenerated through the full scout path.

Startup cost also matters for trivial searches. Ethereal Chains improves bulk engine throughput by 2.6%, but adding its 1.76 ms ranking cost to the idealized work for one result changes 10.76 ms baseline to 12.24 ms selected. Avoid paying for sophisticated ranking on queries with negligible predicted gain; a neutral choice is sufficient for most such seeds.


**The first result in the web UI is a different metric**

The current [web worker](../web/src/lib/search/worker.ts) advances 256 seeds before it can post results; the WASM session also generates batches of 256. For easy queries, both policies usually find a match somewhere in that first batch. More matches per second then improves the number of results returned, but scarcely advances the first visible result.

The analysis therefore also groups eight consecutive native timing blocks into 256-seed delivery batches. `total batch time / nonempty batches` estimates the time to a nonempty batch at that granularity. This is a native model of the web's delivery behavior, not a browser measurement; it excludes worker startup, the 100 ms posting rule, scheduling across workers and rendering. If first-result latency is the main product goal, smaller initial batches and promptly posting the first match would help expose the engine improvement. No such UI change is included in this study.

| Query | Nonempty batches, baseline → selected | Modeled first-batch time saved |
| --- | ---: | ---: |
| +1 Grim Runic Blade, floor 19 | 33 → 54 of 128 | 38.5% (95% interval 21.4–54.6%) |
| +1 Thorns armor, floor 9 | 119 → 127 of 128 | 6.2% (2.1–11.6%) |
| +2 Ring of Might, floor 9 | 120 → 124 of 128 | 3.1% (−0.7–7.0%): inconclusive |
| +1 Grim melee weapon, floor 19 | 64 → 64 of 64 | No demonstrated improvement |
| Ethereal Chains, floor 9 | 128 → 128 of 128 | No demonstrated improvement |

This distinction changes the recommendation: the feature improves engine efficiency for several query types, but a promise of 37–43% faster **visible first results** for the broad Grim/Thorns queries would be misleading with today's web batching. The rare named +1 blade has evidence of benefit under both metrics.

**The policy measured**

The prototype scores all seven generation profiles once per query using the existing probability tables. Adding a selected-trinket AND requirement and dividing its estimate by the exact offer probability, `4/17`, exposes each conditional equipment estimate without modifying production code. These estimates determine the choices before any benchmark seed is inspected. They are estimates of which choice is best for the query, not advance knowledge of a particular seed's contents.

Parchment Scrap, Mimic Tooth, Rat Skull and Cracked Spyglass become preferred choices when their conditional estimate exceeds baseline by at least 5%. This threshold is a fixed guard against calibration noise, not a tuned or proven optimal cutoff. Parchment Scrap is excluded if any requested effect is a curse. Of the preferred trinkets actually offered, the prototype picks the highest-scoring one.

If none is offered, it picks an offered trinket with no searchable generation effect. This includes Exotic Crystals: the existing implementation consumes the conversion float regardless of conversion probability, and the searchable catalog excludes consumables. If all four offers alter searchable generation, it chooses the highest-scoring nonforbidden offer. Thus every benchmark seed receives **exactly one of its four initial offers**, including neutral choices. It never generates a baseline first to decide which offer to use, and never retries a failed seed with another choice.

The baseline is the existing No Trinket search. Each mode calls `generate_batch_gated` once per batch and retains the production query plan's floor pruning, generation horizon and optional vault skipping. Selected effects start after the existing catalyst/alchemy brewing opportunity, at +3. Selection affects future generation; it does not retroactively enchant equipment on already-generated floors.

**Measurement and uncertainty**

The [Rust harness](../crates/seedfinder-core/examples/single_trinket_benchmark.rs), [runner and analysis script](../tooling/benchmarks/single_trinket.py), [environment](benchmarks/single-trinket/environment.json), [rankings](benchmarks/single-trinket/rankings.jsonl) and [machine-readable summary](benchmarks/single-trinket/summary.json) make the study reproducible. Per-query JSONL files in the same directory contain every paired timing block, query document, chosen-trinket counts, overlap counts and replayed example seeds.

Runs use one native engine thread on an eight-vCPU AMD EPYC-Genoa Linux VM, Rust 1.98.1, release optimization with fat LTO. This is a shared host with unrelated background work. Baseline/selection order alternates every 32 seeds; Linux process CPU time is recorded alongside elapsed wall time to check scheduling effects. No other benchmark from this study runs concurrently. These are native engine measurements, not browser/WASM, rendering or multiworker timings.

Both modes scan identical dispersed seeds. Index `i` maps to `(i × 3,355,211,884,971 + 812,345,678,901) mod 26^9`; the main runs begin at index 100,000. Sample sizes were fixed before each run, with no stopping after a favorable result or a requested number of matches. The largest main sample has zero intersections with the existing 200,000-world baseline, 32,768-world trinket, and 4,096-world artifact calibration grids. Ranking tables were already present in the repository and were not fitted to these results. They were measured against BETA-4; this experiment checks the resulting policy on RC-1.

The first nine queries were specified before the main measurements. The named +1 Grim Runic Blade case was added afterward to test a rarer enchantment target where web batching should hide less of the gain; its 32,768-seed sample was fixed before running it. The harness also exposes the previous floor-24 +3 query for optional follow-ups, but that case was not part of this measured suite.

“Seconds per match” is measured engine time divided by matching seeds. It estimates average search work to obtain one result from representative seeds; it is **not a repeated time-to-first-result experiment**. One-time ranking cost is recorded separately, and also added to the one-result estimate in the summary. Parsing, seed construction, telemetry, JSON output and verification replays are outside the timed calls; offer lookup, generation, matching and destruction of generated worlds are inside.

Approximate 95% intervals resample consecutive pairs of timing blocks 2,500 times, preserving baseline/selection hit correlation and alternating execution order. They quantify sampling and observed timing uncertainty, not model parity, browser performance or all possible queries. Sparse rare-query counts deserve particular caution. Each query's first three selection-only examples are independently regenerated through the full scout path, with and without the selected trinket, and asserted to match only with selection.

The `pilot/` directory preserves two completed exploratory runs using an earlier No Trinket fallback. Those rows are excluded from the final analysis; the final policy always chooses an actual offer. The interrupted pilot armor run produced no complete row and contributes no results.

Validation passed: workspace Rust formatting, Clippy with warnings denied for the new example, Python syntax compilation, and the existing selected-trinket integration test containing 21 Java fixture cases. Aggregate and per-block counts were checked for consistency, and all final samples chose an actual offered trinket for every seed.

**Implementation consequences**

- Keep the choice conditional on the whole query, including source, floor, effect and upgrade constraints. More items can help rings, but cannot create a higher upgrade than a source supports. Guaranteed-enchanted quest rewards also limit Parchment Scrap's incremental benefit on some rare +3 queries.
- Mossy Clump and Trap Mechanism can reshuffle which seeds match, as the earlier branching study demonstrated. That alone does not establish improved match probability when only one world is evaluated. Their existing profiles provide no substantial gain for these queries; they are not preferred choices here. Exotic Crystals supplies no searchable-equipment gain in the present model. All three can still be unavoidable or neutral fallbacks under the literal always-pick rule.
- Preserve the selected trinket with each result and replay it in scouting and exports. A result should clearly say, for example, “Choose Mimic Tooth +3.” Picking a different generation-changing trinket can invalidate the match.
- Treat selection policy as part of search-world identity. Tightening a query can change its ranking and therefore which world is generated for a seed. The current `SearchQuery::continues` predicate only compares explicit selection slots; ordinary subset-based filter/resume logic would be unsound if it ignored automatic choices. Preserve original result recipes and invalidate coverage when choice semantics change.
- The existing +3 brewing model assumes the selected trinket is available at its modeled brewing opportunity. It does not simulate whether a particular player's resource spending allows that exact upgrade schedule. Challenges, arbitrary complex OR/combined-level queries, other potency levels and browser performance remain outside this measurement.

Reproduce from the repository root:

```sh
cargo build --release -p shpd-seedfinder-core --features json-query --example single_trinket_benchmark
target/release/examples/single_trinket_benchmark rank
python3 tooling/benchmarks/single_trinket.py run
python3 tooling/benchmarks/single_trinket.py summarize
```

An individual query can be rerun with `target/release/examples/single_trinket_benchmark bench 32768 100000 ring_might`. Use a disjoint index range to collect independent additional evidence. Raw measurements should remain separate rather than pooling reruns of the same seeds.
