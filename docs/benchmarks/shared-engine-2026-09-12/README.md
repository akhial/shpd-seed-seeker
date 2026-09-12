# Shared engine checkpoint measurements

These compact results accompany [the performance report](../../PERFORMANCE-2026-09-12.md). All timings are seconds. They retain every final scored observation, including negative or mixed pairs. Original generated records, executables, profiles and oracle archives remain in the local resume archive identified by the report and provenance hashes.

The direct comparison is original `ae0cb0c07aca55bc4ec31b32f7e6aa15a714a0b0` → merged accepted candidate `fbee876f67835d59691de49fed0f3e0f9ac27bb3`. Subsequent PR commits publish documentation/evidence and bound default CI tests; production engine code is unchanged. Historical comparisons have different explicitly identified revisions and must not be multiplied into a cumulative result.

| File | Contents |
| --- | --- |
| `linux-summary.json` | All raw timing arrays, per-case throughput, paired changes, match counts, environment ranges, provenance limitations. |
| `linux-samples.jsonl`, `linux-timings.csv` | The same 144 scored requests in JSONL/CSV, with input indices, worker counts, equality scopes and full-result signatures. CSV `argv`, where present, is JSON. |
| `linux-design.json` | Literal query JSON, exact comparator commands, seed/count/warmup/order definitions and timer boundaries. |
| `linux-provenance.json`, `linux-audit-provenance.json` | Compiler, target, flags, allocator, source/executable/build/check identities, source-inventory validation, EOF scope and input hashes. The original exact build argv was not saved; its command is explicitly reconstructed. |
| `historical-timings.json`, `historical-provenance.json`, `historical-design.json` | Selected preceding direct/incremental screens, including regressions and WASM results, with query/count/interval/source maps and explicit missing-receipt limitations; these are acceptance history, not final cumulative measurements. |
| `validation.json` | Exact-code Linux checks, oracle replay/control receipts, Mac correctness job and code CI. |
| `ci-tests.json` | Subsequent test-only CI runtime correction, source/log hashes, default debug/release results and explicit extended-sweep results. |
| `SHA256SUMS` | Hashes of this evidence directory's other files, including raw timing exports. |

Mac data:

| File | Contents |
| --- | --- |
| `mac-summary.json` | All four-pair timing arrays and per-query gains/regressions at 1/8/12 workers. |
| `mac-samples.jsonl`, `mac-timings.csv` | All 190 individual requests, including 38 explicitly labelled warmups. CSV is a compact projection; JSONL retains complete audit references. |
| `mac-design.json`, `mac-units.json` | Literal plans, queries, ranges, component/repetition mapping, predeclared selection/counts and unavailable first-attempt disclosure. Old forecast fields are estimates, not observations. |
| `mac-provenance.json`, `mac-completion.json` | Source, plan, archive, runner/auditor and output identities. The completion manifest hashes the original audit outputs under their original filenames; `SHA256SUMS` hashes the published names. |
| `mac-environment.jsonl`, `mac-sessions.jsonl` | Per-job platform/compiler/frozen identities and whole-process resource counters. Those counters include warmup/setup/JSON and peer waiting, not individual internal timed requests. |

The four-case Mac suite is separate from the nine-case Linux suite. Cheap multicore observations sum three components by the same round index; twelve-worker components contain two requests each. Warmups are excluded from summary ratios. The late Mac corpus has no matches, so its timed equality alone does not exercise a positive witness. Full matching equality and CLI count-only equality have distinct scopes.

For each case and worker count, the throughput multiplier is `sum(baseline_seconds) / sum(candidate_seconds)` because the compared inputs have equal counts. The percent change is `100 * (multiplier - 1)`. Take the geometric mean of case multipliers, separately for each host/worker count; do not average seed rates across unlike queries. Four AB/BA pairs are eight timed samples, not four entire ABBA blocks or a statistical confidence interval. Different worker counts use different input totals and do not establish fixed-input strong scaling.

The following recomputes the Linux headline from the raw arrays (Python standard library only):

```sh
python3 - <<'PY'
import json, statistics
from pathlib import Path
cells = json.loads(Path('docs/benchmarks/shared-engine-2026-09-12/linux-summary.json').read_text())['cells']
for workers in (1, 8):
    ratios = [sum(c['baseline_seconds']) / sum(c['candidate_seconds'])
              for key, c in cells.items() if key.endswith(f'/w{workers}')]
    assert len(ratios) == 9
    print(workers, 100 * (statistics.geometric_mean(ratios) - 1))
PY
```

To reproduce timing, use the isolated-build/freezing workflow in [NATIVE.md](../../../tooling/benchmarks/NATIVE.md) with the exact revisions above, then the three commands in `linux-design.json`. Replace only machine-local paths with the corresponding frozen artifacts/output directories. Use the same normal release flags, package graph, allocator and recorded workload source. Never rebuild between alternating samples or run competing builds/tests/profiles during timing. Shared-host conditions and compiler changes can produce different observations.
