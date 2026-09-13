# Repeated mandatory item pruning — Linux evaluation, 2026-09-13

**Retained in `2ab235ce540d9f6faaa5d72a0df4256de7ae90c2`.**
Four affected workloads improve in all 32 paired comparisons, including rings
and armor as well as the original wand targets. The ordinary controls improve
slightly as a group, with an early/eight-worker regression retained below.
Correctness, separate planning-latency measurements and normal-release code
inspection are complete. The search gains justify the measured one-time
planning cost; 13 of 14 planner cases became slower.

This is a direct **incremental** comparison from accepted PR commit
`c47e2f57e524b843c8534de3a92d01f4a0512ea3` to the newly frozen C20B snapshot, integrated byte-for-byte in `2ab235c`.
The baseline engine is checkpoint `fbee876f67835d59691de49fed0f3e0f9ac27bb3`.
The [September 12 report](PERFORMANCE-2026-09-12.md) separately measures original
`ae0cb0c` → `fbee876`. Those tables remain checkpoint results; their gains have
not been multiplied by these incremental ratios or relabeled as new results.

## Change and mechanism

`feasibility.rs` groups identical mandatory single-option requirement slots
using full plan equality and original slot order; `query.rs` adds the required
hash derivations. After every eligible ordinary source has closed, the plan
rejects a prefix if fewer distinct matching item indices exist than required
slots. This avoids generating deeper floors
for an impossible query. Counting remains optimistic about accessibility and
conflicts, which final matching still checks. Nonrepeated single-option slots,
alternative groups, optional sums and later quest possibilities retain their
conservative handling. No canonical generator or RNG implementation changes.

The current production code matches preserved C20B experiment `2938075b`; its
new changes within test code bound default CI and exercise active automatic
trinket selection. The exact patch is SHA256
`57cdf9d6cf012e12541f54420c6b88331cad3db539936a413e955a797996de67`.
In six fixed floor-trace cases, both wand query variants reject two negative
seeds at floor 4 where the reference reaches floor 7 or 9. The two positive
cases retain their floor-24 results. This records 12 avoided completed-floor
callbacks across those fixtures, not a population estimate or timing result.

## All eight workloads

Rates below are seeds/second from total tested divided by total internal time.
Each case/worker has four alternating pairs, AB, BA, AB, BA; all 128 timed
samples lasted 12.31–28.79 seconds. No observation or workload was excluded.

| Query | w1 baseline → candidate | Gain | w8 baseline → candidate | Gain |
| --- | ---: | ---: | ---: | ---: |
| Cheap ring, depth 1 | 10594.56 → 10678.87 | +0.796% | 83156.14 → 83488.80 | +0.400% |
| Early Ghost armor | 2734.84 → 2771.32 | +1.334% | 21304.27 → 21159.82 | **−0.678%** |
| Existing CLI benchmark | 374.49 → 374.79 | +0.079% | 2902.86 → 2937.43 | +1.191% |
| Production blade/Might, automatic trinket | 353.01 → 354.27 | +0.355% | 2703.49 → 2746.15 | +1.578% |
| Six-wand minimum upgrades + Resin | 7713.37 → 8516.41 | +10.411% | 59527.14 → 66012.79 | +10.895% |
| Six-wand exact upgrades + Resin | 7694.94 → 8519.39 | +10.714% | 59651.88 → 65971.38 | +10.594% |
| Two rings by floor 6 + late blade | 294.61 → 555.87 | +88.679% | 2286.10 → 4259.90 | +86.339% |
| Two tier-2 heap armors + late blade | 364.39 → 654.48 | +79.611% | 2826.24 → 5110.96 | +80.839% |

**Eight-case geometric mean: +19.824% / +19.776% at one/eight workers.**
The four ordinary controls alone give +0.640% / +0.619%. Giving the two
six-wand variants one combined family weight yields +21.209% / +21.125% over
seven families. Each cell ratio is `sum(baseline seconds)/sum(candidate seconds)`;
means weight case ratios, not raw seed rates or execution stages. This targeted
suite is not an estimate of the distribution of queries in actual use.

There are **55 positive and nine negative pairs out of 64**. Every affected
pair is positive. Early/eight-worker has paired changes +1.019%, −0.512%,
−1.627%, −1.569%; its pooled regression is not dismissed as noise. Other mixed
cells are cheap/eight-worker (two negatives), CLI/one-worker (two),
CLI/eight-worker (one), and production/one-worker (one). Their complete raw
arrays and signs remain in [summary.json](benchmarks/repeatable-items-2026-09-13/summary.json)
and [TABLE.md](benchmarks/repeatable-items-2026-09-13/TABLE.md).

Historical C18b → C18b+C20B results remain separate: cheap/eight-worker
−1.293% and early/eight-worker −2.319%, both with four negative pairs. The fresh
comparison does not erase them. Earlier whole-process counters used distinct
profiling builds; near-zero instruction/branch changes could not establish
that the normal-release losses were noise or identify a code-layout cause.

## Reproduction and evidence scope

The Linux host reports eight EPYC Genoa KVM vCPUs. Both variants use Rust
1.98.1 / LLVM 22.1.8, portable `x86_64-unknown-linux-gnu`, O3, fat LTO, one
codegen unit and the normal CLI/FFI MiMalloc allocator. Both exact build receipts
select CLI+FFI, `--bin seed-seeker --example match_benchmark`, with locked/offline
Cargo. No `target-cpu=native`, new ISA, PGO, or allocator change is introduced.
Frozen executables are reused between samples; compilation is not timed work.

At w1/w8, literal counts are cheap 196608/1572864; early 49152/393216;
CLI and production 6144/49152; each six-wand case 196608/1048576;
each ring/armor case 8192/65536. Adapter repetition `r` starts at index
`160000000 + r*N`, mapped by `(index*3355211884971 + 812345678901) % 26**9`.
CLI repeats `[0,N)`. Different N across workers is not fixed-input scaling.
Both six-wand cases leave automatic trinket application off; required Resin
identity is distinct from selecting/applying it. Exact query JSON is retained.

The matching timer includes worker setup, generation, matching, witness
selection and per-match JSON construction; query planning and final response
serialization are outside it. CLI includes planning and rounds to milliseconds.
Each matching service has an untimed warmup. The independent audit passed all
six stages: 56 complete seed/recipe/ordered-witness pairs and eight CLI count
pairs. Every case/worker had positive matches, although one repetition of each
one-worker six-wand query returned none. Empty pairs alone do not prove positive
witness coverage. Full adapter wire stdout and warmup records are not separately
retained; pinned lifecycle completion establishes shutdown checks, not an
independent per-process wire transcript.

Separate physical `baseline-source` and `candidate-source` directories share
one sequential Cargo target, so code layout outside the source change is not
assumed identical. The empty override followup was recorded after the baseline
build; it is not a baseline-start capture or hermetic global Cargo inventory.
Shared-host steal, pressure and external-process deltas remain observations,
not explanations or exclusion rules; snapshots can miss short-lived processes.
Four pairs and one VM limit precision. This run establishes no new Mac, WASM,
JNI/System allocator, Android-device or browser throughput claim.

## Planning cost and generated code

The separate setup experiment measures all 14 cases with four alternating
warm process pairs and eight first-call pairs per case. The independent audit
checks all 389 completed processes, including 53 baseline-only calibration
attempts. Warm measurements include `QueryPlan::analyze` and destruction; first
calls exclude separately measured destruction. Query decoding and allocator
availability initialization happen before timing. First calls are not whole-app
cold starts. Three batches within a process are not independent trials.

**13 of 14 warm planning cases regress.** For the six-wand query, warm setup
rises from 0.890 to 1.529 µs (+71.749%); median first-call setup rises from
5.019 to 33.320 µs. The production blade/Might control rises from 7.304 to
7.789 ms (+6.644%, all four warm pairs slower). At 4,096 unique requirements,
warm planning rises from 0.687 to 2.544 ms (+270.291%); at 4,096 repeated
requirements it rises from 0.462 to 1.496 ms (+223.670%). Even the ineligible
controls regress: 16.457 → 18.248 µs at eight requirements and
11.817 → 12.403 ms at 4,096. Their cause is not established; the grouping map
need not execute to observe a regression. The [complete 14-case table and raw
nanoseconds](benchmarks/repeatable-items-2026-09-13/planner/FINDINGS.md) retain
all increases, first-call values and pair signs; no across-query setup average
conceals them. These costs occur per plan, not per seed, and are accepted for
the measured long-running searches. Small requests can have a worse tradeoff.

The original quadratic grouping design was rejected during source review;
the retained implementation uses an expected-linear hash pass with exact key
equality and a stable source-order emission pass. Fewer than two eligible
predicates return before allocation. No hash iteration order affects pruning.

[Inspection of the actual frozen normal-release adapters](benchmarks/repeatable-items-2026-09-13/codegen/REPORT.md)
finds the same 6,100-byte worker body, stack frame and two atomic cursor sites.
After explicit address normalization, only three plan-field displacements differ.
The plan grows from 104 to 128 bytes, and viability grows from 2,154 to 2,502
bytes. Empty groups add a load/test/branch; retained Hall-check register and
branch layout also changes. The cheap depth-one query invokes the new loop
zero times per timed seed. Plan/cursor offsets rule out the proposed new
64-byte cache-line overlap in this adapter, but do not establish a general
absence of contention. No cause for the control regressions is proven and no
speculative layout change is included.

## Correctness validation

Earlier focused release validation passed **18 tests**, explicitly including
the ignored broad multiplicity matrix and the new active-auto fixture. That
fixture covers a seed requiring Spyglass and a seed retaining no recipe, with
full worlds, search/finish, forced choices, filter/refine and canonical replay.
Default fixtures remain bounded; ignored extended sweeps must be run explicitly.

The new larger differential passed **1,788 world pairs across 12 query cases**.
Its reference is the same isolated candidate engine with only the new grouped
capacity check disabled. All **181 retained worlds** and their ScoutMatches
agree; **223 newly returned `None` results** correspond to reference worlds
that fail the query. It also checks **960 recipe inputs through each of filter
and refine**. Both-`None` outcomes do not locate earlier rejection; the separate
floor trace above supplies that narrower observation. This is finite Rust
differential evidence, not a new Java oracle or exhaustive seed-space proof.

The separate frozen-executable adapter differential passes **48 query/worker
comparisons, 130,032 query/seed inputs per variant and 3,677 matching records**.
It compares complete returned seed/recipe/ordered-witness records and replays
returned recipes against full floor-24 generation. It does not compare every
nonmatching full world. No performance conclusion uses its unpaired timings.

The exact, uninstrumented candidate passes:

```sh
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --exclude shpd-seedfinder-gtk --all-targets -- -D warnings
cargo test --locked --offline --workspace --exclude shpd-seedfinder-gtk
cargo test --locked --offline --workspace --exclude shpd-seedfinder-gtk --release
python3 tooling/benchmarks/test_services.py
cargo check --locked --offline --release -p shpd-seedfinder-wasm --target wasm32-unknown-unknown
cargo check --locked --offline --release -p shpd-seedfinder-jni --target aarch64-linux-android
cargo check --locked --offline --release -p shpd-seedfinder-jni --target x86_64-linux-android
```

The core suite has **525 passing tests and seven ignored**, taking **17.24 s
debug / 1.63 s release**. All 13 Python protocol tests pass. The full default
debug workspace command takes 83.87 s including its fresh build. The broad
multiplicity matrix exceeding 1,000 seed attempts remains opt-in; default tests
retain bounded positive/negative, deadline, source, identity, OR and automatic
trinket coverage. These test-runtime observations are separate from throughput.
Cross-target checks compile only. [Validation receipts](benchmarks/repeatable-items-2026-09-13/validation.json)
record exact source hashes, commands, completions and comparison scopes.

All integrated engine files equal the measured/tested snapshot byte-for-byte.
This completes only the requested repeated-item experiment on Linux. Mac was
offline; no new Mac jobs were submitted. Imp-only reservation, collision SIMD
and PGO remain paused. This is not a claim of optimization saturation.

[Compact raw timings, queries, provenance and manifest](benchmarks/repeatable-items-2026-09-13/)
retain all scored observations. Full local records and correctness receipts
remain under `repeatable-20260913/` in the hash-bound performance archive.
