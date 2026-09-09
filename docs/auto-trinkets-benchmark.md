# Auto-apply trinkets: engine benchmark and eligibility study

Measured 2026-09-09 using the shared Rust engine directly, without a browser or WASM adapter. The implementation under test includes Parchment Scrap, Mimic Tooth, Rat Skull, Cracked Spyglass, Mossy Clump, and Trap Mechanism. Each independently applied initial offer uses the canonical +3 activation after catalyst/alchemy availability. Baseline worlds remain eligible and each seed is counted once.

## Method

- Intel Core i5-10400F (6 cores, 12 logical processors), Windows 11 Home 64-bit, build 26200.
- `rustc 1.97.1 (8bab26f4f 2026-07-14)`, `stable-x86_64-pc-windows-gnu`, release profile (`opt-level=3`, fat LTO, one codegen unit).
- One native benchmark process, one engine thread. No search workers, browser scheduling, network, or rendering included.
- The [benchmark harness](../crates/seedfinder-core/examples/auto_trinkets_benchmark.rs) calls `core::auto_trinkets::search_batch` in both modes. It times only that call; parsing, planning, seed construction, and output serialization are outside the timer.
- Every query scans identical contiguous ranges beginning at seed 0 in chunks of 32. Baseline/auto order alternates each chunk. Eight seeds in a separate range warm each mode first. The measured baseline-plus-auto duration reaches at least 120 seconds per query. There is no early termination on finding a match or result cap.
- The five initial cases cover the requested ring and named weapon examples plus armor and an artifact. The rare named weapon cases were extended to 300 seconds each after sparse initial counts. Their extended ranges start at seed 0 again: extended rows supersede the initial rows, and counts are **not pooled**. A sixth, broader +1 Grim weapon query was then added to test Parchment Scrap's mechanism with more matches. The raw log retains every run, including the short initial weapon runs.
- No challenges; upgrades are exact, not minimum. Queries and complete aggregate counts appear in the [raw JSONL output](auto-trinkets-benchmark.jsonl). All six distinct queries are reported, including sparse or unfavorable outcomes.
- A +1 Grim weapon pilot overlapped 6.8 seconds of a web build. That run is explicitly marked `excluded_from_summary` in the raw log and was repeated without the competing build; only the repeat is used below.

Observed seconds per match is elapsed engine time / unique matching seeds. The relative change is `(auto seconds per match / baseline seconds per match - 1) × 100%`. This is a fixed-range throughput estimate of time to obtain a match, **not** a repeated randomized time-to-first-match experiment. Sparse counts have substantial uncertainty. Native ratios need not match browser/WASM or multiworker performance, and these measurements do not establish a universal default for all queries.

## Results

Auto-apply finds additional matching seeds, and every baseline match is retained. The well-sampled ring, armor, artifact, and broader enchantment queries all take more engine time per match with auto-apply. The rare named weapon queries remain sparse even after longer runs. This supports keeping the setting off by default: it expands coverage for users who want the additional trinket recipes, while faster searching is not established by these measurements.

`New` is the number of auto-only seeds. Time columns show baseline → auto. Positive change means slower estimated time per match.

| Query | Paired duration | Seeds per mode | Matches baseline → auto (new) | Measured seconds baseline → auto | Seconds per match baseline → auto | Change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| +2 Ring of Might, floors 1–9 | 120 s | 19,648 | 191 → 257 (+66) | 35.276 → 84.756 | 0.1847 → 0.3298 | +78.6% |
| +3 Grim Runic Blade, floors 1–19 | 300 s | 19,776 | 8 → 13 (+5) | 87.736 → 212.301 | 10.9670 → 16.3308 | +48.9% |
| +3 Grim Runic Blade, floors 1–24 | 300 s | 14,880 | 6 → 9 (+3) | 88.139 → 211.874 | 14.6899 → 23.5416 | +60.3% |
| +3 Plate Armor, floors 1–19 | 120 s | 11,968 | 9,449 → 9,681 (+232) | 52.445 → 67.733 | 0.00555 → 0.00700 | +26.1% |
| Ethereal Chains, floors 1–9 | 120 s | 21,632 | 3,011 → 3,256 (+245) | 37.270 → 82.874 | 0.01238 → 0.02545 | +105.6% |
| +1 Grim weapon, floors 1–19 | 120 s | 7,968 | 274 → 506 (+232) | 35.892 → 84.172 | 0.1310 → 0.1663 | +27.0% |

The Ring of Might case gains **34.6% more matching seeds per sampled range**, but costs **78.6% more seconds per match**. The broader Grim query gains **84.7% more matches**; Parchment Scrap supplies 173 of its 232 additional matches, confirming its intended enchantment benefit. Its added generation work still makes the estimated seconds per match 27.0% worse. All six included trinkets contribute auto-only matches somewhere in the measured cases.

The extended named weapon results have only eight and six baseline hits, respectively; treat them as sparse estimates, not reliable percentage predictions for a future search. Their shorter initial runs are retained in the raw log for transparency and are not combined with the extended ranges. In the floor-19 example, the first accepted seed shifts from 1,346 to 590 (using Trap Mechanism), showing a concrete seed the baseline query misses; that earlier numeric seed is not by itself a measurement of faster time to the first result.

## Why include Mossy Clump and Trap Mechanism?

Both yield searchable matches that baseline generation misses. The scanner generates each offered trinket alone and compares it with the same seed without a selection. These examples are also exercised by core regression tests:

| Trinket | Seed | Requirement missed by baseline | Applied result |
| --- | --- | --- | --- |
| Mossy Clump | `AAA-AAA-AAC` (2) | Uncursed Cleansing Dart by floor 6 | Floor-6 shop |
| Trap Mechanism | `AAA-AAA-AAF` (5) | Exactly +1 Explosive Spear by floor 7 | Floor-7 mimic |

Mossy Clump changes floor feelings and skips a later override draw when its override succeeds. Trap Mechanism changes floor feelings and trap revelation. These changes can alter subsequent map generation and equipment draws; their value is not limited to users searching for floor feelings. See [`roll_feeling`](../crates/seedfinder-core/src/level_prelude.rs) and the raw [fixture output](auto-trinkets-fixtures.txt).

## Why exclude Exotic Crystals?

The fixture study scans seeds 0–999, generating through floor 24. Of those seeds, **224 offer Exotic Crystals initially**. Selecting it produces **zero differences in the complete searchable `GeneratedWorld`** compared with baseline across those 224 seeds: items, effects, upgrades, sources, accessibility, quests, feelings, and ring gems all compare equal.

This result agrees with the current implementation: [potion/scroll conversion checks](../crates/seedfinder-core/src/generator.rs) consume the same float whether the conversion probability is zero or nonzero. [Crystal Path duplicate detection and sorting](../crates/seedfinder-core/src/special_consumable.rs) compare the underlying consumable identity, independent of exotic conversion. Potions and scrolls are not searchable equipment in the current query model. This is evidence of no present search benefit, not a proof about all future engine versions; revisit eligibility if the searchable model or RNG handling changes.

## Reproduce

From the repository root, on a machine with a working Rust linker:

```powershell
cargo +stable-x86_64-pc-windows-gnu build -p shpd-seedfinder-core --features json-query --release --example auto_trinkets_benchmark
target/release/examples/auto_trinkets_benchmark.exe fixtures
target/release/examples/auto_trinkets_benchmark.exe bench 120
```

The recorded run placed build artifacts on `D:` using `CARGO_TARGET_DIR=D:\codex-auto-trinkets-target`; that only changes the executable path. Other platforms can omit the explicit Windows toolchain and `.exe` suffix. Increase `120` to lengthen every query's paired measurement. An optional third argument filters query names; `bench 300 grim_runic` reproduces the two extended rare cases and `bench 120 grim_weapon` runs the broader follow-up alone. Fixture generation and timing use direct Rust APIs, so this harness does not require a web build.
