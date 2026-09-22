# Duplicate-wand calibration

The estimator previously applied a correction measured against independent
Poisson draws after its matcher had already enforced single-choice rewards.
This counted some of the same restrictions twice. The six reported duplicate-
wand outliers underestimated success rates by 33–54%; a separate 756-query sweep
found 81 failures of the documented tighter calibration check.

The replacement measures the probability of obtaining two, three, or four copies
of one wand. It keeps the upgrade threshold, dungeon depth, and the two groups
of wand identities that can or cannot appear in the vault. Separate conditions
cover one upgraded copy and one copy from the Wandmaker, Imp reward, or vault.
The bake intersects complete acquisition-scenario masks, so mutually exclusive
rewards are never counted together.

At runtime, the engine divides that measured probability by the estimate for
the same broad condition, using the same reward-aware matcher, then applies the
ratio to the requested filters. Source, curse, exact-level, and individual
floor constraints remain in the query's matching calculation. This also handles
uneven groups such as one +4 wand alongside two plain copies. Broad intermediate
answers use the existing bounded cache; no worlds are sampled at runtime.

Resin planning uses the same calibrated wand supply. Excluding a wand from Auto
resin removes its upgrade cost while still reserving that copy; the Mage's
starting wand contributes two resin without adding a generated wand. Exact-floor
probabilities multiply the resulting item-and-resin estimate once per profile.
`tests/resin_floor_probability.rs` checks this composition, cache isolation,
selected and automatic trinkets, and equivalent fixed and Auto budgets at +1,
+2, and +3. These checks preserve the calibration rules; they do not establish
new accuracy bounds for the joint resin model.

## Data and coverage

The canonical profile uses 262,144 training worlds. Each of the seven trinket
profiles uses 65,536, for 720,896 worlds total. Seed offset 4669201609 and stride
3355211884971 are recorded in the bake reports. The first two floors reuse the
larger canonical sample because brewing cannot affect them.

The WDR2 table occupies 147,492 bytes and is read directly. Each count records
how many identity/world pairs offered the requested copies; estimates average
within each vault-eligibility group. Cells with fewer than 30 observations,
more than four copies, or unsupported conditions retain the previous analytical
fallback. Arbitrary combinations of multiple different filters, unequal early
limits, and curse/source interactions remain approximations.

The first independent sweep covers 756 linked and named wand queries: counts,
early deadlines, minimum upgrades, curses, and sources. A further 672-query
sweep adds exact upgrades, +4 wands, and combinations with quest sources. Each
uses 65,536 separate validation seeds, at offsets 2302585093 and 693147180.
All 758 sufficiently sampled cases pass the unchanged four-sigma Wilson interval
expanded by a factor of 1.35; 670 are underpowered.

The same 1,428-query corpus was checked for each selected trinket over 65,536
seed trials at offset 1414213562, including its initial-offer probability.
Across all eight profiles, the [wand audit](benchmarks/probability-calibration/wand-summary.json)
contains **11,424 comparisons: 5,381 pass, 6,043 are underpowered, none diverge**.
These validation streams share no seeds with the wand calibration stream.
The fixtures retain 112 representative selected-profile cases as well.

The regression fixtures also retain 57 linked-wand queries from the earlier
262,144-world random sweep. The original six duplicate-wand failures now pass.
The wider 7,102-query random audit has six remaining tighter-band failures,
all involving other equipment or early-floor competition.

| Original query | Old estimate, 1 in | New estimate, 1 in | Observed, 1 in |
| --- | ---: | ---: | ---: |
| Four matching wands by 21; one by 5, another ≥+2 | 1,061.6 | 568.8 | 642.5 |
| Three by 16; one uncursed | 90.5 | 60.4 | 60.6 |
| Three by 19; one by 8, another ≥+2 | 22.2 | 16.9 | 14.8 |
| Three by 11; one ≥+1 from a chest | 2,458.7 | 1,139.0 | 1,120.3 |
| Three by 16; one by 15, another uncursed by 9 | 97.7 | 64.7 | 64.3 |
| Four by 21; one by 7 | 348.9 | 206.5 | 214.3 |

## Performance and verification

The 200-query native release timing check peaks at 4.6 ms against the unchanged
50 ms budget. In the dedicated wand audit, the canonical sweep has sub-millisecond
p95 latency; selected profiles stay below 1 ms p95 and 13 ms maximum. These are
native timings, not browser frame guarantees. The WASM payload grows by roughly
150 KB raw and 23 KB gzip, with no runtime simulation or table initialization.

All 740 Rust workspace tests and 272 web tests pass, along with strict Clippy,
format/type checks, table-packing tests, and the bake's acquisition-scenario
test. Fixtures retain 1,597 wand observations. The browser preview loads the
updated engine without errors and shows the three-wand, one-uncursed floor-16
example as approximately 1 in 60 (previously 1 in 90.5; observed 1 in 60.6).

## Reproduce

```sh
cargo run --release -p shpd-seedfinder-core --example calibrate_wand_repeats -- 262144 /tmp/wands-none.json
cargo run --release -p shpd-seedfinder-core --example calibrate_wand_repeats -- 65536 /tmp/wands-mimic.json mimic_tooth
python3 tooling/probability/pack_wand_repeats.py /tmp/wand_repeats.bin /tmp/wands-none.json /tmp/wands-mimic.json
cargo test -p shpd-seedfinder-core --test probability_wand_repeats
cargo test -p shpd-seedfinder-core --example calibrate_wand_repeats
```

Repeat the bake for the remaining trinket IDs before installing the production
table. Partial diagnostic tables use the analytical fallback for missing
profiles; production integrity tests require all eight. The overnight runner
now includes all eight wand bakes and packs their table without installing it.

See the [general calibration report](probability-calibration.md) for the complete
audit, tolerance definition, and broader model limitations.
