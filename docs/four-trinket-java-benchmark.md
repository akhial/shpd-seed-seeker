# Four-trinket search versus Java

Measured on 2026-09-24 on an 8-vCPU AMD EPYC-Genoa Linux host. Seed Seeker used
an AVX2 release build with fresh query-specific PGO; Java used the pinned
Shattered Pixel Dungeon v4.0.0 desktop JAR and OpenJDK 21.0.12.1. Each side
ran eight workers for ten warmed, timed minutes. One mode occupied the CPU at a
time, with mode order reversed on successive blocks. Timings include batch
protocol overhead. Both searched dispersed numeric seeds from the benchmark's
fixed seed formula, starting at index 1,000,000.

The attached query requires all four initial catalyst offers: Dimensional
Sundial, Rat Skull, Parchment Scrap, and Petrified Seed. At the user's request,
the dark garden or secret garden condition moved from depth 17 to depth 7, and
depth 4 must have the GRASS feeling to keep result counts below 1,024. The
query retains `max_depth: 22`, `auto_apply_trinket: true`, and
`exclude_blacksmith_rewards: true`. Explicit trinket requirements disable
automatic trinket selection, and the last required floor is depth 7.

| Engine | Timed seeds | Matches | Seeds/second | Matches/minute |
| --- | ---: | ---: | ---: | ---: |
| Java game JAR | 69,365,760 | 21 | 115,609 | 2.10 |
| Seed Seeker AVX2 | 1,761,816,576 | 584 | 2,936,344 | 58.40 |

Seed Seeker searched **25.4×** as many seeds per second and observed **27.8×**
as many matches per minute. The latter ratio is less precise because Java found
only 21 matches. A simple independent rare-event approximation gives a 95%
interval of about 18–43× for the observed match-rate ratio; it does not include
host-load or implementation uncertainty. The two samples' match densities were
0.303 and 0.331 per million seeds, respectively.

The Java adapter checks the four initial offers before generating floors, so
both engines avoid floor generation for seeds whose offers cannot match. It
checks GRASS at depth 4, then DARK and the presence of either garden room at
depth 7. Three known positive seeds were checked against both engines before
timing. At the user's request, **equivalence tests and post-run replays were
skipped** for this benchmark. Thus these timings do not establish full
Java/native match-set parity.

Reproduce with:

```sh
tooling/benchmarks/run-four-trinkets-linux.sh --minutes 10 --workers 8 \
  --output target/benchmark/four-trinkets-depth7-20260924
```

The [machine-readable summary](benchmarks/four-trinket-garden/summary.json) is
committed. The run's raw blocks, environment, source hashes, binary hash, and
profile hash are under `target/benchmark/four-trinkets-depth7-20260924/`
(ignored by Git). The official JAR SHA-256 was
`b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1`.
