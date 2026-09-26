# Artifact transmutation probability

Artifact searches with a transmutation limit now use the remaining deck at the
requirement's floor limit and require an obtainable starting artifact. The same
Rust estimator serves web, Android, macOS, Windows, Linux, and AutoTrinket.

## Model

`artifact_decks.bin` contains 8,192 generated layouts for each of eight profiles
(65,536 training worlds). Each layout stores artifact floor/source/curse records
and maximal simultaneously obtainable subsets. The calibrator checks that every
floor's generated artifacts plus remaining deck contain exactly eleven cards.
Inaccessible alternatives still consume deck cards, but cannot supply two donors
at once. Artifact identities are integrated analytically without replacement;
the model does not estimate rare named combinations by counting training hits.

A single requirement averages natural matching positions plus reachable remaining
deck positions when a suitable donor exists. Joint requirements assign distinct
identities and donors, preserving the matcher's floor-boundary and accessibility
rules. Identical filters use falling factorials. Other layouts share cached
assignment states; bipartite donor matching takes polynomial time. A bounded work
budget protects the editor for unusually complex imported queries.

The model separates runs that can satisfy artifacts without taking the Imp prize
from runs that need it. Equipment can claim the prize only in the first case.
This matters for combinations such as Skeleton Key within two transmutations and
Ring of Energy +4. AutoTrinket ranks profiles using these same estimates and
averages its actual offer-selection policy.

Runtime estimation performs no world generation or random sampling. The 972,878-byte
ADP1 table embeds as 445,217 compressed bytes and decompresses once. Training uses
offset 314159265 and stride 3355211884971. SHA-256 of the checked-in table:
`27a113a9c64d7f9c524eede48f286257dc4ba9019e16b54d8500387059c0e932`.

Remaining approximations: artifact identity is treated as exchangeable within
anonymous layouts; unrelated equipment/room supply uses the existing analytical
model. Challenges retain canonical estimates. Imported artifact-transmutation
upgrade filters remain unavailable because rounding through intermediate artifact
identities has not been calibrated; normal artifact editors use any upgrade.
Scroll availability and later-generation changes remain outside search semantics.

## Held-out validation (2026-09-26)

The [79-query corpus](benchmarks/probability-calibration/artifact-queries.json)
covers six floor limits, five transmutation budgets, source/curse filters, multiple
artifacts, different horizons, OR/blanket requirements, and competition with
upgraded equipment. Each query was checked against 65,536 seed trials using a
separate offset (2718281828). Worlds are reused across queries. Selected-profile
trials include initial-offer availability; AutoTrinket trials generate only the
profile chosen for that seed, rather than taking the union of possible worlds.

The [report](benchmarks/probability-calibration/artifact-summary.json) includes all
eight profiles plus AutoTrinket: **711 query/profile comparisons**. All **693
sufficiently sampled comparisons pass** the existing four-standard-error Wilson
check with a 35% model allowance. The other 18 are the same two structurally
impossible cases in each profile, with both predicted and observed probability
zero: an Imp reward before floor 9, and two competing Imp rewards. The generic
report labels zero/zero cases underpowered; separate regressions assert their
impossibility. No thresholds were widened.

For AutoTrinket, estimate/observed ratios have median 0.997, p10 0.973 and p90
1.006. For the canonical profile they are 0.996, 0.953 and 1.006 respectively.

| Skeleton Key ≤2 + Ring of Energy +4, floor 24 | Estimated | Observed | Matches / 65,536 |
| --- | ---: | ---: | ---: |
| AutoTrinket on | 1 in 61.03 | 1 in 59.31 | 1,105 |
| AutoTrinket off | 1 in 62.65 | 1 in 59.04 | 1,110 |

Native release estimates for that AutoTrinket query took 1.36 ms. Across its
79-query audit, median was 1.06 ms, p95 11.24 ms, maximum 27.63 ms. In the T3
browser, the rebuilt WASM estimated the screenshot query in 19.2 ms on first use
(after loading WASM, including probability-table initialization), then 0.1 ms from
cache. These are observed timings, not universal device guarantees. The rendered
editor shows **Match probability ≈ 1 in 61**.

CI uses 1,024 held-out seeds, reused across twelve queries, plus finite-estimate,
monotonic-budget, duplicate-identity, and Imp-exclusivity regressions. Large audits
are explicit release-mode examples and do not run in CI.

## Reproduce

```sh
cargo run --release -p shpd-seedfinder-core --example calibrate_artifact_decks -- 8192 /tmp/artifact_decks.bin 314159265
cargo run --release -p shpd-seedfinder-core --example probability_sweep -- 65536 none 2718281828 docs/benchmarks/probability-calibration/artifact-queries.json > /tmp/artifact-none.json
cargo run --release -p shpd-seedfinder-core --example probability_sweep -- 65536 auto 2718281828 docs/benchmarks/probability-calibration/artifact-queries.json > /tmp/artifact-auto.json
python3 tooling/probability/report.py --check /tmp/artifact-none.json /tmp/artifact-auto.json
```

Repeat with `mimic_tooth`, `parchment_scrap`, `rat_skull`, `exotic_crystals`,
`mossy_clump`, `trap_mechanism`, and `cracked_spyglass` in place of `none` to check
all selected profiles. The complete local observations are retained under
`/tmp/artifact-review/probability-*.json` for re-estimation with `--recheck`.
