# Shared engine performance checkpoint — 2026-09-12

This checkpoint records accepted Rust engine work from `ae0cb0c07aca55bc4ec31b32f7e6aa15a714a0b0` through `2ee20c3f597fb7f00872a1858f59d9424a14d16b`. The accepted engine changes are merged with current `main` in measured candidate `fbee876f67835d59691de49fed0f3e0f9ac27bb3` (v0.12.1). Investigation pauses at the user's request for a stable, mergeable PR. This is **not a claim of optimization saturation**: credible, untested avenues remain.

The scope is the shared engine and measurement/parity tooling. No frontend optimization, new ISA requirement, normal-release PGO dependency, or `target-cpu=native` requirement was introduced. Compatibility remains pinned to Shattered Pixel Dungeon 4.0.0; see [COMPATIBILITY.md](COMPATIBILITY.md) and [EQUIVALENCE.md](../tooling/parity/EQUIVALENCE.md).

The later [September 13 repeated-item evaluation](PERFORMANCE-REPEATABLE-2026-09-13.md) records the separately measured incremental change from `c47e2f5` to `2ab235c`. The tables below remain the original checkpoint results.

## Measurement and provenance

The original worktree was clean. Its environment/build log records compiler, release settings, target and allocator; the exact initial shell argv was not separately saved, so the displayed original build command is a reconstruction. The final candidate has an exact command receipt. Original source, executable hashes, release executables, and separate profiling executables were frozen before experiments. Each candidate was saved separately, allowing alternating measurements without rebuilding between samples. Rejected candidates and raw observations were retained.

| Environment | Configuration and scope |
| --- | --- |
| Linux | Eight KVM vCPUs reported as AMD EPYC Genoa; Rust 1.98.1 / LLVM 22.1.8; portable x86-64 target. One and eight workers. |
| macOS | Mac16,8, ARM64, 48 GiB; macOS 26.6.2; Rust 1.98.0 / LLVM 22.1.8; portable `aarch64-apple-darwin`. One, eight, and twelve workers where recorded. |
| WASM | `wasm32-unknown-unknown`, normal release settings, `wasm-pack --no-opt`, no `simd128`; Node 24.21.0 with `--no-liftoff --no-wasm-lazy-compilation`. One worker; this measures Node/V8, not browser performance. |
| Production release | `opt-level=3`, fat LTO, one codegen unit, repository allocator and target settings. CLI and the linked FFI matching adapter use MiMalloc; WASM uses its normal allocator. JNI/Android retain their shipping allocator/build settings and were not independently benchmarked. |
| Profiling | Separate repository profiling builds with symbols; explicitly recorded package/feature build graphs for comparative code inspection. Linux hardware sampling/counters and V8 profiles were collected separately from release timing. |

The suite includes the existing CLI `--benchmark` (+5 Runic Blade by floor 19), the production matching benchmark, cheap ring searches, early Ghost and wand constraints, late/deep equipment queries, vault queries, crossbow queries, and production blade/Might searches with and without automatic trinket selection. Query JSON, seed families, worker counts, and literal sample counts are preserved with each run. [NATIVE.md](../tooling/benchmarks/NATIVE.md) and [native_workloads.json](../tooling/benchmarks/native_workloads.json) describe the reusable benchmark protocol and workload catalog.

Most candidate screens used four alternating AB/BA pairs; initial baseline calibration used three samples. Short or inconsistent cases received longer controls where possible. Internal search timers exclude compilation and process startup. The production matching timer includes matching, witness construction, and construction of per-match JSON values; final response serialization is outside it. CLI durations have millisecond rounding. Multi-request sums are identified as sums, not continuous runs.

Returned matching records were checked for seed, recipe, and ordered-witness equality, with only documented outer-order/timer normalization. CLI timing checks compare counts, not full worlds. WASM timed recipe-array comparisons and the separate complete scouting comparisons have different validation scopes.

## Accepted changes and invariants

| Cycles | Change and measured mechanism | Compatibility constraints |
| --- | --- | --- |
| C02 | Cache collision distances, stable-sort once, and traverse while clipping. Removes repeated nearest scans and collision-vector removal copies. | Preserve containing-rectangle prechecks before tie draws, stable tie order, wrapping distance arithmetic, `i32::MAX` sentinel behavior, and current-space intersection. |
| C04, including C03 | Telescope smoothing population changes; avoid the unused initial population reduction on the packed patch path; count final rows before fill correction. | Same initial random decisions, smoothing output, target fill, correction decisions, and subsequent RNG state. Scalar fallback remains. |
| C05 | Advance the three unused maze direction draws with exact affine RNG states. | Bound-three rejection is checked explicitly; rejected cases use the original draw sequence from untouched state. Draw count/order and child/subsequent state are preserved. |
| C07 | Test terrain/patch eligibility before room eligibility in private canonical painting dispatchers. | Public custom dispatchers default to the original callback order, including out-of-range and side-effect behavior. |
| C08w | Use an exact 32-bit reciprocal quotient and correction on WASM, removing the expensive wide multiplication helper. | Full unsigned remainder domain and Java bounded-draw rejection remain exact; native retains its original arithmetic. |
| C09–C10 | Reject impossible mandatory named-trinket requests at initialization; compute only the four offer identities needed by search. | Clone the private deck; preserve the full public diagnostic deck, original generator state, actual offers, OR semantics, and optional-sum conservatism. |
| C14n | Skip empty bounds and factor overlap tests in native direct-room collision scans. | Dense scans and the WASM implementation retain their existing path. Signed extremes, inverted/empty rectangles, invalid-ID traversal, stable scratch behavior, and RNG remain covered. |
| C15–C16 | Refine Imp/vault source possibility with pinned identities and joint tier/upgrade/effect inventory rows. | No independent-bound approximation may omit a feasible row. Optional sums remain conservative; shop and quest sources retain their distinct rules. |
| C17 | Close ordinary tier-two and tier-three generation opportunities at their pinned natural deadlines. | Shop, vault, Smith, quest, OR, and optional-sum exceptions are handled separately. This is query pruning, not a change to canonical world generation. |
| C18b | Defer independent vault generation until mandatory non-vault requirements survive, under a narrow eligibility guard. | Capture original insertion position, choice group, depth, and trinket effects. Materialize surviving worlds in original order. Any typed deferred-path error retries the original eager path. Custom gates remain eager by default. |
| C19d | Pack/unpack eight Boolean cells at a time on targets other than x86-64 using safe, explicit byte conversions. | Preserve all bits, tails, sentinels, and endianness; no new unsafe code. Keep the original x86-64 SSE/scalar-tail production implementation. |

Matched C17/C18b profiling captures attributed 13.7–13.9% of production auto-trinket cycles to vault construction before deferral and 0.18–0.36% afterward. Total sampled production cycles fell about 13.9% / 14.3% at one/eight workers. These were separate profiling builds; their timings do not replace normal release comparisons. The retained clean captures use 32 KiB DWARF stacks, 499 Hz at one worker and 199 Hz at eight, after higher-rate multicore captures lost samples. All original loss diagnostics remain archived. For C19d, Mac whole-process instruction counts fell about 2.27% for CLI and 2.26–2.47% for matching controls; matching counters include setup, warmup and JSON work. This evidence supports reduced work without predicting a universal cycle saving.

Default full-world/scouting generation and surviving deferred-vault results preserve complete item/choice order. Query-gated internal generation may omit irrelevant vault contents or stop at an earlier sound horizon, as its existing contract permits; it should not be described as byte-identical to an unconditional 24-floor world. A query-proven rejection may avoid a later generation error, as earlier pruning already could. Surviving-seed error behavior and default custom callback behavior remain covered separately.

Supporting tooling adds native scheduler/session adapters, stricter process/EOF and result validation, reusable workloads, and arbitrary-range oracle capture/replay with negative controls. Scheduler defaults, chunk sizes, cancellation semantics, and result limits were not tuned merely for throughput.

## Final direct baseline-versus-checkpoint measurements

Direct comparison of the original baseline against the final merged candidate, with separately frozen normal-release executables. These are cumulative measurements, not products of incremental cycle ratios. The checked-in [evidence directory](benchmarks/shared-engine-2026-09-12/) contains every compact raw timing, query, count, full-result signature, build provenance and historical comparison used below. Large original world records, binaries and profiles remain in the hash-bound local archive.

### Linux: nine workloads, one and eight workers

**Equal-weight nine-case geometric mean: +19.23% with one worker and +18.42% with eight. All 72 paired changes were positive.** For each case/worker, throughput ratio is total baseline seconds divided by total candidate seconds over equal input counts; aggregate is the geometric mean of those ratios. Giving the two blade/Might modes one combined family weight instead gives +17.36% / +16.74%. No samples were removed.

| Query | w1 baseline → candidate seeds/s | Gain | w8 baseline → candidate seeds/s | Gain |
|---|---:|---:|---:|---:|
| cli | 316.34 → 372.54 | +17.77% | 2445.24 → 2837.18 | +16.03% |
| cheap | 9699.84 → 10431.09 | +7.54% | 74195.36 → 81403.97 | +9.72% |
| early | 2331.65 → 2723.37 | +16.80% | 18311.55 → 21101.19 | +15.23% |
| wand | 749.09 → 889.14 | +18.70% | 5929.51 → 6931.14 | +16.89% |
| late | 215.89 → 249.03 | +15.35% | 1680.70 → 1924.08 | +14.48% |
| vault_feasible | 271.69 → 311.66 | +14.71% | 2141.97 → 2414.40 | +12.72% |
| blade_might | 259.24 → 350.44 | +35.18% | 2022.73 → 2708.37 | +33.90% |
| blade_might_auto | 254.23 → 344.39 | +35.46% | 2032.83 → 2675.14 | +31.60% |
| crossbow | 314.44 → 359.98 | +14.49% | 2463.58 → 2893.41 | +17.45% |

All 144 timed requests lasted 10.833–19.631 seconds. Each cell has four alternating pairs (AB, BA, AB, BA), with an untimed warmup. The cheap query uses 131,072 / 1,048,576 seeds per one/eight-worker request; early uses 32,768 / 262,144; Wandmaker uses 12,288 / 98,304; each other query uses 4,096 / 32,768. Adapter repetition `r` starts at index `144000000 + r * count`, mapped as `(index * 3355211884971 + 812345678901) % 26**9`. CLI repeats `[0, count)`. Eight-worker requests use eight times the input, so these are throughput observations, not fixed-input strong scaling.

Independent audit checked all 64 full adapter seed/recipe/ordered-witness pairs and eight CLI count-only pairs, exact tested counts/order/EOF, full original and candidate source inventories, frozen hashes and build provenance. There were 7,225,344 scored query/seed inputs per variant, including repeated CLI intervals. Every query/worker cell returned at least one match except late/one-worker. Late/eight-worker returned one exact matching witness across its four samples; empty rare-query samples still perform real generation but do not establish nonempty witness coverage by themselves.

No per-workload regression was observed in this final Linux run. Four pairs are limited evidence, not confidence intervals or a universal speedup guarantee. The shared VM observed about 0.147–0.164% guest steal with one worker and 1.223–1.328% with eight; no cgroup throttling. Persistent external-process CPU deltas were 0.19–1.57 seconds per sample (at most 1.36% of whole-host capacity). Snapshots cannot detect every short-lived process or establish the cause of timing differences. Adapter wire stdout and full warmup records were not separately retained: successful completion of the hash-bound strict lifecycle implementation proves the shutdown checks, not an independent replay of that wire transcript.

### Mac: four workloads, one, eight and twelve workers

**Equal-weight four-case geometric mean: +7.43% / +4.96% / +7.84% at one / eight / twelve workers.**

| Query | 1 worker | 8 workers | 12 workers |
| --- | ---: | ---: | ---: |
| Existing CLI benchmark | +5.24% | +1.06% | +6.29% |
| Any ring, depth 1 | +0.70% | -0.26% | +2.23% |
| Heap Runic Blade +2 Grim, depth 24 | +5.05% | +2.96% | +3.19% |
| Production blade/Might with automatic trinket selection | +19.64% | +16.95% | +20.61% |

The only pooled regression is **cheap/eight-worker −0.26%**, with paired changes −0.31%, +2.83%, −2.64%, −0.80%. CLI/eight-worker is +1.06% overall but ranges from −5.06% to +9.07% across pairs. These small, variable changes remain visible; they were not discarded or attributed to an unmeasured cause. Other cells also have occasional negative pairs, retained in `mac-summary.json`.

Measured final production-query throughput is 686.79 / 2,764.37 / 4,555.49 seeds/s at one/eight/twelve workers. The observed eight/one and twelve/one rate ratios are 4.03× and 6.63×. Per-query baseline and final absolute rates are retained in `mac-summary.json`. These different-input measurements show that scaling depends on the worker count; they do not isolate scheduling, core placement or bandwidth as the cause.

Counts per complete one/eight/twelve-worker sample are CLI 13,824 / 96,512 / 114,432; cheap 387,328 / 2,780,928 / 3,412,224; late 8,960 / 63,744 / 63,744; production 11,008 / 75,264 / 75,264. Mac requests use consecutive actual seeds and repeat the same inputs for all four pairs, unlike the dispersed Linux intervals. Complete query JSON and every component interval are in `mac-design.json` and `mac-units.json`.

Cheap/eight-worker sums three separate requests/jobs per round; cheap/twelve-worker sums six requests across three jobs. Their individual requests last 6.55–9.04 and 3.72–5.14 seconds respectively. They receive one query weight in the aggregate, not three or six extra independent repetitions. Other requests last 15.38–35.42 seconds. No fixed-input scaling claim follows from different worker counts/input sizes. Mac reports eight performance cores and twelve available cores; worker count does not prove core placement, clock, temperature or QoS residency.

All eight archive-bound jobs passed complete source/compiler/runner/build/frozen-executable, result, readiness/order/EOF and shutdown auditing. The collector and a separate compact recalculation agree on 16 comparison units, 190 records (38 warmup and 152 timed requests), and four paired complete samples in each of twelve cells. CLI exposes counts only. All matching records agree in full within each job; repeated equal inputs across jobs also agree by canonical record hash, byte count and tested count. Cheap and production queries return matches (production has 4 / 25 / 25 per sample), while the Mac late query returns none in this corpus. Its equality check therefore does not provide positive-witness coverage. Separate focused and oracle validation remains necessary.

One cheap/eight-worker component's first job completed execution but could not deliver its archive. The identical validated plan was rerun, and only that fully available, audited retry is included. The unavailable attempt is recorded in `mac-design.json` and Macqueue issue MQ-009; this was an availability-based retry, not timing-based sample selection. Each job independently built/froze both revisions before alternating them; exact executable hashes are retained per job without claiming independent builds were byte-identical.

Some staged evidence is useful for understanding acceptance, but is not the final cumulative result. The checked-in `historical-design.json` maps all 42 published historical timing cells to their queries, counts, intervals and source identities; unavailable old C17 receipts and legacy shutdown limitations are explicitly marked. C02's nine-workload aggregate improved approximately 4.65% at one worker and 4.59% at eight. C05 retained a repeated 1.10% cheap/eight-worker loss alongside substantially better deep searches. C10 retained a 1.14% long CLI/eight-worker loss. C17 retained a 1.40% long cheap/eight-worker loss; some WASM suite-control losses did not repeat in longer isolated controls. Those observations remain in the ledger.

An earlier direct original-to-C18b Mac screen recorded cheap/one-worker −0.84% with all four pairs negative, CLI/one-worker +3.28%, CLI/eight-worker −0.55% with mixed signs, and CLI/twelve-worker +5.36% with mixed signs. Its four measured cells are not a representative-suite aggregate; they remain visible beside the final measurements.

C19d versus C18b on Mac improved the three measured one-worker cases by 1.21% geometric mean, with all twelve pairs positive. Its cheap/eight-worker screen improved 2.39%, but samples were only 6.96–7.63 seconds and drifted. The eight-case WASM aggregate was +2.38%, with several mixed-sign cases. Neither result establishes an all-query/all-machine guarantee.

## Correctness validation

Repeated checkpoints ran the requested workspace checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --exclude shpd-seedfinder-gtk --all-targets -- -D warnings
cargo test --locked --workspace --exclude shpd-seedfinder-gtk --release
```

C19d passed 510 core tests with one intentionally ignored test, plus the workspace consumers, on Linux and Mac. Focused differential tests cover collision clipping/order and full RNG state, maze rejection boundaries and fallback, packed/scalar patch output and subsequent state, reciprocal boundary and randomized arithmetic cases, every eight-bit conversion pattern, offset/tail/sentinel behavior, source inventory/deadline boundaries, initial offers and recipes, and eager/deferred vault worlds, errors, and default callbacks.

Java parity capture/replay repeatedly covered the first 10,000 seeds after generation/RNG changes. Later accepted checkpoints also passed four fresh 10,000-seed shards spanning 1,310,000–1,349,999, with zero errors or deviations and exercised negative controls. Oracle comparison includes supported item, position, terrain, and vault data; it is finite fixture/oracle evidence, not exhaustive proof over every seed.

C19d's separate WASM scouting validation compared 207 original-baseline/candidate pairs (414 generated worlds) with complete returned scouting JSON equality. This complements native tests and does not mean the timed WASM search protocol itself compared full worlds.

Source review found no concrete merge blocker in the accepted range. The merged candidate passed formatting, workspace Clippy, release and debug workspace tests (510 core tests, one ignored), all 13 Python protocol tests, and release checks for WASM plus both Android JNI targets. Its normal release comparator replayed seeds 0–9,999 against the retained Java oracle with zero deviations/errors; all nine comparator controls passed. Macqueue also passed the merged candidate's full formatting/Clippy/release-test job. Platform CI status is recorded in the PR.

### Bounded default CI tests

The initial expanded parity matrices made the local core debug suite take 614.92 seconds. After the user identified this CI regression, five broad sweeps were made opt-in: the source-refinement search/recipe matrix, both trinket-preflight matrices, and the two broad deferred-vault matrices. Their full seed/query/challenge/recipe coverage remains available with the release-mode command in [README.md](../README.md#testing).

Default CI retains all focused boundary/state/error checks, two small trinket checks, and a three-seed eager/deferred comparison with a rejection, surviving vault blade, and retained Mimic Tooth recipe. Canonical wrap/cancel/cap tests use 263 seeds with the same partial-chunk, nonempty-result, duplicate-resume and cap-progress assertions. Numerical RNG/collision/patch differentials remain enabled.

On the same devbox, the updated default core debug suite passed **508 tests with six ignored in 17.51 seconds**; the whole debug workspace command took 61.63 seconds including an 11.52-second build. The default core release suite passed in 1.63 seconds, and all five broad library sweeps passed explicitly in release mode in 57.83 seconds. Formatting, Clippy and both debug/release workspace checks passed. These are test-runtime observations, separate from engine throughput benchmarks; [the validation receipt](benchmarks/shared-engine-2026-09-12/ci-tests.json) retains commands, summaries and source/log hashes. Through `c47e2f57e524b843c8534de3a92d01f4a0512ea3`, all changes after the measured engine revision are documentation or code inside test modules. The subsequent production change in `2ab235c` is measured separately in the [repeated-item report](PERFORMANCE-REPEATABLE-2026-09-13.md).

## Rejected and unfinished work

All percentages below are local candidate-versus-accepted comparisons from their own runs; they are not cumulative. Raw samples, source snapshots, and codegen/profile reports remain in the experiment archive.

| Experiment | Evidence and disposition |
| --- | --- |
| C01 distance caching alone | Three-case geometric means +0.37% / +1.13% at one/eight workers, inconsistent per query. Extra records/pass were unjustified alone; combined successfully with C02 stable traversal. |
| C03 smoothing count alone | Longer eight-worker vault control −1.99%, all four pairs negative; subset aggregate −0.73%. Rejected alone, then combined with C04 uncounted initial fill. |
| C06 paired maze coordinate draws | Aggregate −0.06% / +0.03%; common coordinate path grew from 44 to 47 instructions. Extra bounds/branches and code complexity did not establish a gain. |
| C08 native reciprocal | Native profile cycles increased about 2.36%; extra corrected-index checks/code costs. Rejected natively; exact WASM-only variant retained. |
| C11 collision outlining | Aggregates −0.21% / −0.54%; intended invariant-load hoisting did not occur. Extra callee/frame work was unjustified. |
| C12 compact collision order | One-worker aggregate −1.03%; WASM +0.72% mixed. Extra scratch, indexing, and larger native code outweighed smaller sortable records. |
| C13 empty skip alone | Long cheap/one-worker −0.93%, all four negative; WASM aggregate −0.20%. Rejected; C14's factored native combination was evaluated separately. |
| C14 on WASM | No supported improvement; kept original WASM predicate and accepted only the native variant. |
| C18c static Imp guarantee | Intended target −1.69% / −1.27% natively and −0.21% on WASM. Small mixed control aggregates did not justify the change. Restored C18b. |
| C19/C19b/C19c portable path on x86-64 tails | C19 cheap/eight-worker −4.31%, all four negative. Borrowed conversion and iterator-remainder refinements improved codegen but did not establish robust native gains; C19c aggregates −0.78% / −0.99%. C19d therefore retains original x86-64 code. |
| C20B closed duplicate-slot capacity | Excluded at this checkpoint: target six-wand searches improved on native/Mac, but Linux cheap/eight-worker −1.29% and early/eight-worker −2.32%, all four negative, remained unresolved and broader acceptance was incomplete. Subsequently retained in `2ab235c` after a fresh Linux comparison, planning-cost measurement and larger differentials; see the [September 13 report](PERFORMANCE-REPEATABLE-2026-09-13.md). Those historical losses remain recorded. |
| C20A Imp-only reservation certificate | Outside source/test draft only; intended to broaden safe deferred-vault eligibility. Not compiled, tested, benchmarked, or accepted. |
| C21 dense collision SIMD | Outside source/proof/differential-test and isolated-build preparation only. Requires current hotspot attribution, codegen, portability validation, and measurement. Not accepted. |
| PGO | Broad Mac training/profile merge and held-out correctness completed; measured matching gains exist. CLI measurement overlaps training, some multicore samples are short/variable, and deployment across consumer build graphs is unresolved. No production profile/configuration accepted. |

Other retained avenues include direct initial patch-bit generation, cheaper patch threshold networks, rolling terrain rows, branch-angle reuse instrumentation, door-connection snapshot allocation removal, room compaction, and earlier Ghost/Imp reward gates. These need current cost attribution and bounded experiments; draft presence is not evidence of speedup. Scheduler sampling found little direct synchronization cost in the inspected workloads, so no default chunk/padding change was justified.

## Limits and resume handoff

The Linux host had virtualization steal and occasional unrelated processes. Known competing task-owned work was serialized; contaminated exploratory observations and diagnostic limitations were retained rather than silently discarded. Host activity, sampling loss/unwind limitations, and counter coverage vary by recording and are described in their audits. Compiler, allocation, code layout, frequency, and scheduling can affect controls; timing alone does not prove an alignment cause.

Mac timings were prioritized after the user identified that host as free of competing work, but per-core placement, temperature, clock, and QoS residency were not measured. Twelve-worker and multi-request results must retain their observed drift and shorter component durations. ARM assembly and process counters corroborate removed work, not a universal cycle prediction. JNI, Android hardware, browsers, and other CPUs were not independently throughput-tested.

A strip-only companion did not always have identical `.text` to its production executable. Such companions were not treated as a valid symbol map for production code. C19d's actual normal x86-64 CLI/adapter code, layout, relocations, and constants were separately verified identical to C18b, with only panic-line/build-ID metadata differences; this is limited to those frozen artifacts.

The working archive is `/home/adel/.cache/seed-perf-ae0cb0c-results`; `/tmp/seed-perf-ae0cb0c` is its compatibility symlink. It is a local resume archive, not a public repository artifact. Preserve it or export the relevant evidence before deleting the workspace. Compact raw results and a hash manifest are checked in under `docs/benchmarks/shared-engine-2026-09-12/`; interpreting the published timings does not require the machine-local archive. Resuming unfinished experiments still requires that archive.

| Handoff | Location or identity |
| --- | --- |
| Accepted cutoff | `2ee20c3f597fb7f00872a1858f59d9424a14d16b` |
| Immutable original / accepted binaries | Archive `baseline/`, `candidates/c18b/`, `candidates/c19d/`, with SHA/source receipts |
| Experiment chronology | Archive `ledger.md`, `raw/`, `profiles/`, per-cycle preparation and audit directories |
| Frozen benchmark semantics | Archive `tooling-snapshots/lifecycle-v1-8f9600e/`; repository `tooling/benchmarks/` |
| C19d evidence | Archive `mac-c19d-independent-findings/`, `c19d-wasm-audit/`, `c19d-normal-elf-identity/` |
| C20B followup | Historical draft `2938075b4e04d969880d9c40b1e043fd320ef074` and `candidates/c20b/` remain preserved; their old measurements compare C18b+B against C18b. The subsequent current-checkpoint combination was measured directly against `c47e2f5` and retained in `2ab235c`; current evidence and handoff are in archive `repeatable-20260913/` and the [September 13 report](PERFORMANCE-REPEATABLE-2026-09-13.md). |
| C20A draft | Archive `c20a-current-worktree-01/`: patch, inverse patch, source, provenance, review; based on C19d plus pending B. Preserve B's reference that disables only duplicate groups. |
| C21 preparation | Archive `c21a-preparation/` and current collision SIMD patch/proof. Isolated baseline is accepted C19d; old full-builder drafts are stale. |
| PGO | Archive `mac-c17-pgo-broad-retry-*` and `mac-c17-pgo-training-audit/`; retain training/holdout distinctions and exact profiles/build graph. |

Resume by reading the ledger and artifact receipts, verifying source/binary hashes, and choosing the next evidence-backed cycle. C20B is now accepted as recorded in its later report; do not silently treat unfinished C20A/C21 or PGO as accepted, and do not replay old build scripts against a changed worktree without reviewing their preconditions.
