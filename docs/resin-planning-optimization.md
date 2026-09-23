# Resin planning follow-up to PR #158

PR #156 changes which wands need resin. The shared planner now skips donor-only floors and vaults when every kept wand is excluded or already +3; bare reforge copies remain required but add no upgrade cost. It also skips donor generation when the Mage's starting credit covers a fixed budget.

The fixed donor bound now subtracts that credit before rejecting a seed. Without this adjustment, combining #156 with #158 could prune valid matches. The matcher, probability model, and planner share the same credit calculation. Finally, late floor requirements participate in the generation horizon before the donor bound is built, so floor-filter searches can reject insufficient early supply.

## Validation

637 tests passed across the core unit suite and the arcane_resin, floor_requirements, resin_floor_probability, resin_probability, resin_blankets, and resin_blanket_probability integration suites; nine existing opt-in tests stayed ignored. Strict core Clippy with all targets and workspace formatting passed.

The planner comparisons retain the existing four/five known seeds, include positive and negative matches, and check selected items and trinket recipes with automatic trinket selection both enabled and disabled. New cases cover named and linked reforge stacks, alternatives that still need resin, the Mage credit boundary, and later floor conditions. The optimized test profile and 1,024-case CI limit are unchanged.

## Local performance measurements

Compared the previous PR head `ed62c5f` with this follow-up on 2026-09-23. Both CLI binaries used Rust 1.98.1, the Windows GNU toolchain, the repository's release profile (O3, fat LTO, one codegen unit), and MiMalloc on an Intel Core i5-10400F (6 cores / 12 logical processors). Automatic trinket selection was disabled in these timing queries; its correctness remains covered by the comparisons above.

Each cell ran one warm-up pair and three measured pairs, alternating binary order. Each invocation searched 1,024 inputs with one or four workers. Values are median seeds/second; percentages compare the two medians. Every pair returned identical match counts. These are targeted query gains, not a general engine speedup; controls and negative differences are retained below.

| Query | Matches / 1,024 | 1 worker: before → after | 4 workers: before → after |
| --- | ---: | ---: | ---: |
| Excluded Auto wand by floor 4 | 641 | 285 → 1,805 (+533.3%) | 1,074 → 6,382 (+494.2%) |
| +3 Wandmaker anchor with a reforge copy by floor 4 | 8 | 785 → 1,016 (+29.4%) | 2,735 → 3,299 (+20.6%) |
| Two fixed resin covered by Mage credit, ring by floor 4 | 585 | 321 → 1,831 (+470.4%) | 1,073 → 6,521 (+507.7%) |
| Six resin by floor 4, dark floor 22 | 4 | 286 → 1,287 (+350.0%) | 895 → 4,299 (+380.3%) |
| Six resin by floor 4 with Mage credit, dark floor 22 | 15 | 289 → 880 (+204.5%) | 1,027 → 2,866 (+179.1%) |
| Control: Auto Wandmaker wand | 1024 | 196 → 194 (-1.0%) | 660 → 683 (+3.5%) |
| Control: six resin by floor 4, Wandmaker wand | 78 | 1,622 → 1,620 (-0.1%) | 5,694 → 6,135 (+7.7%) |
| Control: +5 Runic Blade by floor 19 | 18 | 336 → 344 (+2.4%) | 1,174 → 1,188 (+1.2%) |

Reproduce each row with `seed-seeker --items QUERY.json --benchmark 1024 --workers 1` (then `--workers 4`) using the following query documents, in table order:

```jsonl
{"arcane_resin":"auto","requirements":[{"kind":"wand","max_depth":4,"exclude_resin":true}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":"auto","requirements":[{"kind":"wand","upgrade":3,"source":"wandmaker_reward","identity_group":1},{"kind":"wand","identity_group":1,"max_depth":4}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":2,"arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"kind":"ring","max_depth":4}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":6,"arcane_resin_filter":{"max_depth":4},"requirements":[],"floor_requirements":[{"depth":22,"feeling":"dark"}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":6,"arcane_resin_filter":{"max_depth":4,"include_mage_wand":true},"requirements":[],"floor_requirements":[{"depth":22,"feeling":"dark"}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":"auto","requirements":[{"kind":"wand","source":"wandmaker_reward"}],"max_depth":24,"auto_apply_trinket":false}
{"arcane_resin":6,"arcane_resin_filter":{"max_depth":4},"requirements":[{"kind":"wand","source":"wandmaker_reward"}],"max_depth":24,"auto_apply_trinket":false}
{"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":5}],"auto_apply_trinket":false}
```

Local raw outputs and the comparison driver are retained under `target/pr156-optimization/` (ignored build artifacts).
