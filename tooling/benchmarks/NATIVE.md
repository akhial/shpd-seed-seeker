Use `compare_native.py` with two already-built, frozen native directories. Each directory needs `seed-seeker` and `match_benchmark`. Keep the sibling `native_workloads.json` beside the script, or select it with `--workloads`.

Record the actual build provenance before copying the executables: revision and any source patch/snapshot hash, `rustc -vV`, target triple, Cargo arguments and features, build profile, `RUSTFLAGS`, allocator, and any PGO configuration. Representative engine builds use release opt-level 3, fat LTO, one codegen unit, and the normal production allocator and target configuration. Record each variant separately. Do not infer build settings from the host compiler available when measuring. The optional `--build-metadata` JSON is copied into the run metadata; its schema is unrestricted, so it can contain both variants' existing build records. The harness never builds binaries or requires PGO artifacts.

Run one measurement job at a time, with no builds, tests, profilers, or other benchmarks competing for the machine. Freeze both executables separately before sampling and keep them unchanged; their hashes are checked before and after the run. Use a fresh output directory for every invocation.

```sh
python3 /absolute/repo/tooling/benchmarks/compare_native.py \
  --baseline-dir /absolute/frozen/baseline \
  --candidate-dir /absolute/frozen/candidate \
  --output /absolute/results/native-abba-01 \
  --service-module /absolute/repo/tooling/benchmarks/effective_matches.py \
  --build-metadata /absolute/frozen/build-metadata.json \
  --workers 1,8 --reps 4 --scale 1 --start 1000000
```

The default suite contains `cli,cheap,early,wand,late,vault_feasible,blade_might,blade_might_auto,crossbow`. `--cases` selects a comma-separated subset. Four repetitions produce four paired comparisons in AB, BA, AB, BA order: eight timed samples per workload and worker count. Counts are fixed by workload, multiplied by `--scale` and the requested worker count; no adaptive calibration runs. Increase `--scale` explicitly if the recorded durations are too short for reliable comparisons.

For a Mac, choose distinct worker counts covering one worker, the machine's reported performance-core count, and its total logical CPU count. For example, a machine reported as four performance cores and six efficiency cores could use `--workers 1,4,10`. These are thread counts, not CPU affinity settings: the OS selects placement, and four workers do not prove exclusive use of performance cores. The harness neither pins threads nor clamps requested workers to the available CPU count. Record the machine topology and power/thermal conditions alongside results; the default `1,8` may not suit every Mac.

A small protocol smoke run can use:

```sh
python3 /absolute/repo/tooling/benchmarks/compare_native.py \
  --baseline-dir /absolute/frozen/baseline \
  --candidate-dir /absolute/frozen/candidate \
  --output /absolute/results/native-smoke-01 \
  --service-module /absolute/repo/tooling/benchmarks/effective_matches.py \
  --cases cheap --workers 1 --reps 2 --seeds 16
```

`--seeds` overrides the **total** seeds in each sample, including multicore samples, and also bounds warmup. Smoke timings are not performance evidence. For adapter workloads, `--start` is an index into `effective_matches.seeds_at`, whose values are dispersed across the seed space. The complete measured index interval must fit within one `26**9` period; crossing that boundary is rejected. CLI `--benchmark` always scans the contiguous interval `[0,count)` and does not use `--start`.

Inspect these outputs:

- `metadata.json`: arguments, queries, compiler/platform information, build provenance, executable/script/suite/service hashes, exact counts, and measurement limits.
- `samples.jsonl`: compact timing, tested counts, match counts, and signatures.
- `environment.jsonl`: before/after environmental counters keyed by case, workers, repetition, and variant, without large witness records. Linux `/proc` and cgroup fields are optional and normally absent on macOS.
- `comparisons.jsonl`: per-pair signature comparisons; adapter equality also requires exact canonical JSON equality.
- `raw.jsonl`: complete adapter match recipes and witnesses, CLI text/counts, and raw sample metadata.
- `summary.json`: per-workload timings, throughput ratios, paired ratios, regressions, and equal-weight geometric means. Zero or rounded-to-zero durations are explicitly excluded from ratios.
- `completion.json` or `failure.json`, plus adapter setup records and stderr logs.

Use internal search seconds for engine comparisons. Wall seconds also include process/protocol overhead and the inherited `Service.request` validation; this script's additional validation and hashing are outside the wall timer. Adapter setup is untimed, but each request creates worker threads. CLI samples launch a new process and its reported time rounds to milliseconds. Warmup does not preserve worker-local allocator/TLS state across those new threads or processes.

Adapter comparisons cover the complete reported recipe/witness records. **CLI equality is match-count-only**, because the existing `--benchmark` interface does not expose individual results. This harness does not run Java or replace Rust correctness tests and oracle/equivalence validation. Review per-workload and per-worker regressions even when the geometric mean improves.
