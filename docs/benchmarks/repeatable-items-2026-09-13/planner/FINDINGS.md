# Planner audit: all 14 cases retained

PASS: 389 completed processes, exact query/header/body/EOF, zero exits and empty stderr; 53 baseline calibration attempts, 112 warm and 224 fresh processes. Independently recomputed warm and first-use summaries agree.

Positive time change means slower planning. Warm = analyze plus drop; first = analyze before separately timed drop. Values are microseconds.

| Case | Warm baseline → candidate µs | Time change | Slower warm pairs / 4 | Median first baseline → candidate µs | Slower first pairs / 8 |
| --- | ---: | ---: | ---: | ---: | ---: |
| cheap | 0.265 → 0.245 | -7.517% | 1 | 6.750 → 4.662 | 2 |
| blade_might_auto | 7304.123 → 7789.442 | +6.644% | 4 | 6951.025 → 7534.847 | 7 |
| six_min | 0.890 → 1.529 | +71.749% | 4 | 5.019 → 33.320 | 8 |
| six_min_auto | 0.881 → 1.545 | +75.458% | 4 | 4.917 → 31.958 | 8 |
| unique-1 | 0.250 → 0.256 | +2.727% | 3 | 3.175 → 6.971 | 6 |
| unique-2 | 0.363 → 0.865 | +138.334% | 4 | 7.196 → 30.180 | 8 |
| unique-8 | 1.036 → 4.239 | +309.282% | 4 | 4.673 → 35.593 | 8 |
| unique-64 | 7.312 → 35.866 | +390.547% | 4 | 12.805 → 69.010 | 8 |
| unique-512 | 60.646 → 298.520 | +392.232% | 4 | 98.273 → 359.108 | 8 |
| unique-4096 | 687.096 → 2544.256 | +270.291% | 4 | 590.472 → 2490.988 | 8 |
| repeated-2 | 0.358 → 0.893 | +149.820% | 4 | 6.074 → 35.138 | 8 |
| repeated-4096 | 462.048 → 1495.512 | +223.670% | 4 | 401.983 → 1370.880 | 8 |
| ineligible-8 | 16.457 → 18.248 | +10.878% | 4 | 21.707 → 21.247 | 6 |
| ineligible-4096 | 11816.861 → 12402.846 | +4.959% | 3 | 10128.484 → 10863.725 | 7 |

Raw times, all signed process-pair changes, first-call/drop arrays, exact queries, loop counts and provenance pointers are retained alongside this report. No across-query aggregate is used to hide regressions. Ineligible and production-control increases have no established cause; do not dismiss them or infer the duplicate-group hash map executed there.

This measures the analyze-only mode, not the FFI analysis ABI including JSON decode. Fresh processes are initialized and decoded before timing; they are not application cold-start measurements. Three batches per process are not independent trials. Signature parity covers three public plan diagnostics only, while full correctness is separate.

The complete build/source/query/driver maps are in provenance.json; full raw stdout/stderr/intent/execution and process environment remain in the original hash-bound planner-primary-01. Query documents are preserved for all 14 cases.
