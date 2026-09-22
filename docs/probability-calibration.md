# Probability calibration

Probability estimates use baked generator measurements and analytical rules.
They do not scout extra worlds while editing a query or searching, and estimates
never decide whether a seed is rejected.

## Calibration data

The canonical item supply and all seven selected-trinket profiles now use
1,000,000 worlds each. These tables are baked into the engine; the browser does
no calibration work. The larger audit also exposed relationships that more
samples alone cannot fix:

- Blacksmith melee and thrown weapons share their upgrade and enchantment roll.
- Vault identity exclusions apply to both named items and unnamed identity groups.
  Vault wands/rings have distinct identities and one item at each level.
- Vault equipment enchantments depend on the reward shelf, jointly with tier and
  upgrade. The lowest shelf cannot offer enchanted equipment.
- Disjoint quest offers can provide a fallback identity, while the player still
  takes only one reward. Overlapping filters retain nested reaches.
- Canonical weapon duplicates use scarcity measured separately by line and tier,
  counting only mutually compatible item subsets (500,000 training worlds).
- Shops stock one weapon from each line, at the region's fixed tier. Their stock
  is not three independent draws from the weapon generator.
- +3 chest equipment uses next-region treasure tiers and clears curses; these
  properties are evaluated jointly instead of multiplying unrelated marginals.
- Independent item lines and depth stretches retain their own arrival budgets.
  Fractional budgets interpolate valid integer distributions, preserving mass
  and avoiding negative tails or NaNs at guaranteed arrival probabilities.

Source-specific count distributions use another 65,536 worlds per profile.
The 276,428-byte SCF1 table stores the probability of each obtainable item count
by source, generator line, and depth prefix. It captures clustering that mean
slot counts miss, such as two rings lying directly on the floor. Queries with
one shared source and depth limit use these distributions unless they require
a repeated identity. Other query shapes retain the analytical arrival model.

The overnight floor bake contains 16 disjoint shards of 65,535 worlds for each
of eight profiles: **1,048,560 worlds/profile, 8,388,480 total**. The merge sums
integer counts by room identity, rebuilding sparse row indices instead of
assuming the shards share an index layout. `merge_floors.py` accepts the original
u16 FLP3 shards and current u32 FLP4 shards, producing FLP4 without truncation.

The 3,909,168-byte `floors.bin` stores feeling counts, room presence, within-floor
room intersections, repeated room presence across floors, and cross-floor
feeling pairs. Constants need no pair entries. Counts and offsets support
direct reads without decompression, runtime initialization, or dependencies.

Ordinary profiles pool feelings with identical room scheduling to reduce noise.
Mossy Clump and Trap Mechanism instead use separate room distributions for all
eight feelings, baked over another 524,288 worlds each. Brewing timing makes
feeling and laboratory placement dependent even when the feeling does not
change room scheduling itself. Their additional 1,691,904-byte FLX1 table shares
the sparse room-row format. Other profiles use exact feeling probabilities: normal on
floor 1, then 1/2 normal and 1/14 for each special feeling.

First-floor cross-family competition uses 2,000,000 worlds, counting
co-obtainable subsets with chest/scenario choices preserved. Broad queries use
measured presence directly, including a common minimum upgrade of +1, +2 or +3;
filtered queries interpolate between presence and factorial-moment correlations.
Higher-upgrade rows need at least 30 training hits; otherwise the estimator
falls back to the broader model. This table covers mixed equipment families
with at most four requirements, all limited to floor 1.

Two-room queries use measured joints; wider conjunctions/unions use a maximum
strength dependency tree. Cross-floor corrections account for repeated room
features and the alternating feelings of Mossy Clump/Trap Mechanism. Unmeasured
cross-floor interactions, item/floor dependencies, and wider room combinations
remain approximations. Challenges still use the canonical estimates.

## Validation

`examples/probability_sweep.rs` streams generated worlds through exact matching,
retaining only hit counts. It covers every room ID and feeling on every regular
floor, conjunctions/unions, repeated rooms, every farming-floor subset, mixed
farming/item searches, equipment levels and sources, named items, artifacts,
competing requirements, and vault identity exclusions. Selected-trinket sweeps
include the probability that the trinket appears in the initial offers.

The overnight validation uses offset 2718281828 and stride 3355211884971,
separate from the training streams. Earlier regression observations use offset
918273645. The random item-query test uses a separate evenly spaced grid and
configurable query seed. `FUZZ_REPORT` saves exact counts and serialized queries
for rechecking after model changes without repeating generation.

`tooling/probability/report.py --check` compares predictions with a four-standard-
error Wilson interval expanded by a 35% model allowance. It checks zero-hit
queries when at least 30 hits were predicted. Cases with fewer than 30 observed
and predicted hits are reported as underpowered, **not** counted as passes.
This does not certify rare combinations such as all three farming floors.

## Results (2026-09-22)

The [curated report](benchmarks/probability-calibration/summary.json) records
**38,872 query/profile comparisons**: 4,859 queries for each of eight profiles,
each evaluated over 262,144 seed trials. All 23,266 sufficiently sampled
comparisons pass the stated divergence check. The other 15,606 are underpowered,
including many structurally impossible room/source queries with zero predictions
and matches. All original overnight curated failures are resolved.

The wider sweep generated 8,000 random item queries, of which 7,102 were valid,
over a separate 262,144-world grid. All pass the existing factor-two-plus-sampling-
noise regression guard, now extended to catch sparse or zero-hit overestimates
when at least 30 hits were predicted. Among the 4,048 queries with at least 12
observed hits, the median estimate/observed ratio is 0.997, p10 0.931, p90 1.042.

The [stricter random-query report](benchmarks/probability-calibration/random-summary.json)
also applies the curated 35% allowance: 3,982 pass, 3,108 are underpowered, and
**12 remain outside that band**. These include linked wand duplicates, early-floor
competition, and some overlapping equipment filters. Its sufficiently observed
queries have median ratio 0.996, p10 0.932, p90 1.036, and mean absolute log error
0.0387. These remaining model errors are recorded rather than hidden by widening
the calibration threshold.

A further [15-query source-count probe](benchmarks/probability-calibration/source-count-summary.json)
used 65,536 separate worlds: all 13 sufficiently sampled cases pass; two are
underpowered. The regression fixtures retain 678 room/reward/random observations
from these and earlier sweeps, alongside three direct first-floor checks.

| Farming floor | Estimated 1 in | Observed 1 in | Matches / 262,144 |
| --- | ---: | ---: | ---: |
| 7 | 95.5 | 96.7 | 2,710 |
| 17 | 80.5 | 82.6 | 3,175 |
| 22 | 70.8 | 70.2 | 3,734 |

On the local x86-64 release build, baseline estimate latency was 1.2 µs median,
18.1 µs p95, and 2.21 ms maximum across the curated sweep. Selected-trinket
profiles had p95 below 0.43 ms and maximum below 12.5 ms. The random sweep had
35.5 µs median, 0.163 ms p95, and 10.5 ms maximum. The separate 200-query speed
regression measured a 4.5 ms maximum against its unchanged 50 ms release budget.
These are native timings, not browser frame guarantees.

The matcher compacts surplus stock only after each stream has spent its arrival
budget: a coverage needs no more items than it has eligible requirements. This
preserves matching while reducing the costly convolution between streams. The
previous slowest debug estimate fell from 868 ms to 68 ms.

The browser WASM is 10.58 MB (4.08 MB gzip). Baked tables trade a larger initial
download for direct lookups without runtime simulation or initialization. The
updated Tailscale preview loads without browser errors and reports farming
floor 17 as approximately 1 in 80.

## Overnight artifacts

The completed overnight run and morning integration are retained locally at
`~/.local/share/seed-seeker/calibration/overnight-20260922T010707Z` and
`~/.local/share/seed-seeker/calibration/morning-20260922`. The morning directory
contains original observations, refreshed reports, source snapshots, hashes,
bake logs, verification logs, and an integration manifest. The original run
remains unchanged. See the [verification record](benchmarks/probability-calibration/verification.json)
for test counts, the release timing check, and the preview smoke test.

`tooling/probability/overnight.py OUTPUT` runs at reduced CPU priority and writes
only into OUTPUT. It snapshots the executables and records their hashes, exact
commands, offsets, exit codes, and progress in `manifest.json`. Each completed
artifact is renamed atomically. Validation failures are retained for review.
It never installs new tables into the source tree or updates the preview.

The default run calibrates 1,000,000 worlds per item profile (8,000,000 total),
2,000,000 first-floor worlds, and 16 disjoint floor shards of 65,535 worlds per
profile (8,388,480 total). The original floor shards used u16 counts. Integration
sums their integer counts into u32 storage; it does not average query predictions or install one
shard. Current calibrators write u32 counts directly. Each shard's
seed offset is recorded in the manifest.

Fresh validation uses 262,144 seed trials per profile and 8,000 random item
queries. It assesses the current estimator; the new overnight measurements
are installed only after review and held-out rechecking. The runner also bakes
weapon repeats (500,000 worlds), exact-feeling room rows (524,288 worlds for each
of two profiles), and source-count distributions (65,536 worlds for each of
eight profiles), and saves random-query counts. `--plan` prints the full job
list without running it. Jobs continue after an individual failure, preserving both
successful data and diagnostic logs.

## Reproduce

From the repository root:

```sh
python3 tooling/probability/merge_floors.py /tmp/floors.bin /path/to/run/floors-*.bin
cargo run --release -p shpd-seedfinder-core --example calibrate_floors -- 524288 /tmp/feeling-rooms.bin 1618033988 exact
cargo run --release -p shpd-seedfinder-core --example calibrate_weapon_repeats -- 500000 > /tmp/weapon-repeats.rs
cargo run --release -p shpd-seedfinder-core --example calibrate_first_floor -- 2000000 > /tmp/first-floor.rs
cargo run --release -p shpd-seedfinder-core --example calibrate_source_counts -- 65536 /tmp/source-counts.bin
cargo run --release -p shpd-seedfinder-core --example probability_sweep -- 65536 none > /tmp/probability-none.json
cargo run --release -p shpd-seedfinder-core --example probability_sweep -- 65536 mossy_clump > /tmp/probability-mossy.json
python3 tooling/probability/report.py --check /tmp/probability-none.json /tmp/probability-mossy.json
FUZZ_WORLDS=262144 FUZZ_QUERIES=8000 FUZZ_SEED=20260922 FUZZ_REPORT=/tmp/random.json cargo test --release -p shpd-seedfinder-core --test probability_fuzz fuzzed_queries_track_sampled_seeds -- --ignored --nocapture
FUZZ_RECHECK=/tmp/random.json cargo test --release -p shpd-seedfinder-core --test probability_fuzz fuzzed_queries_track_sampled_seeds -- --ignored --nocapture
```

Other profiles: `mimic_tooth`, `parchment_scrap`, `rat_skull`, `exotic_crystals`,
`trap_mechanism`, `cracked_spyglass`. Re-estimate previously measured observations
without regenerating worlds using `probability_sweep --recheck /tmp/probability-none.json`.
