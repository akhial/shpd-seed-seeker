# Java baseline seed finder (v4.0.0)

The Java side of the benchmark in the top-level `README.md`: a seed finder that
runs Shattered Pixel Dungeon's own generator on the JVM, so Seed Seeker's
throughput can be compared with the game's own code **at the version Seed
Seeker targets**.

The [README benchmark](../../README.md#benchmarks) measures matching seeds per
minute, checks the full Java seed sample against native search, and replays
returned trinket recipes independently in the JAR.

This finder drives the **unmodified official JAR** headlessly, using the same
technique as `tooling/oracle-4.0`. Upstream now publishes the full
[v4.0.0 source](https://github.com/00-Evan/shattered-pixel-dungeon/tree/v4.0.0)
at commit `2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`; using the shipped JAR keeps
the baseline tied to the released build without maintaining source patches.

The final-release pin follows [PR #118](https://github.com/akhial/shpd-seed-seeker/pull/118):

- artifact: `ShatteredPD-v4.0.0-Java.jar` (build 912)
- URL: `https://github.com/00-Evan/shattered-pixel-dungeon/releases/download/v4.0.0/ShatteredPD-v4.0.0-Java.jar`
- sha256: `b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1`

`build.sh` reuses the oracle's download when there is one, verifies the sha256,
and compiles `src/**/*.java` plus the oracle's `TextureFilm` stand-in into
`.work/classes`. `run.sh` builds when needed and runs
`com.shatteredpixel.shatteredpixeldungeon.JarSeedFinder` with `classes` first on
the classpath, then the JAR. Both honour `JAVA_21_HOME`, then `JAVA_HOME`, then
`PATH`. The current effective-match driver invokes `java` from `PATH` directly
and records its version and JVM options; `--java /path/to/java` selects another JVM.

## Build and run

```sh
tooling/java-finder/build.sh
tooling/java-finder/run.sh --seeds 2000
tooling/java-finder/run.sh --item WandOfFireblast --upgrade 3 --floors 24 --seeds 500
tooling/java-finder/run.sh --seeds 10000 --warmup 0 --print-matches > matches.txt
```

| Option | Meaning |
| --- | --- |
| `--item CLASS` | item class name (default: `RunicBlade`); `RunicBlade,RingOfMight` supports the +2/+2 floor-19 benchmark |
| `--effect NAME` | comma-separated allowed effects for the first item (default: any) |
| `--stream` | persistent JSON-lines batch search / full recipe replay |
| `--upgrade N` | required true upgrade; `-1` accepts any (default `5`) |
| `--floors N` | deepest floor generated (default `19`) |
| `--seeds N` | timed seeds (default `2000`) |
| `--start N` | first numeric seed (default `0`) |
| `--warmup N` | untimed seeds searched before timing starts (default `200`) |
| `--challenges N` | challenge bit mask (default `0`) |
| `--no-vault` | skip the Imp's Vault |
| `--skip-boss-floors` | step over the state-neutral boss depths 5, 10, 15 and 25 |
| `--print-matches` | print each matching seed code before the `BENCH` line |

Each run ends with a `BENCH` line containing the tested seed count, matches,
elapsed seconds and seeds per second.

## What it searches

Per seed it calls the game's own `Dungeon.initSeed()`, `Dungeon.init()` and
`Dungeon.newLevel()` for floors 1..N, then — when the Imp has spawned — builds
the Imp's Vault (branch 1 of the Imp's floor), and scans everything generated
for a searchable item (weapon, armor, wand or ring) of the wanted class at the
wanted `trueLevel()`:

- every heap and its containers, on main floors and in the vault;
- mimics' contents, statues' weapons and armor, the sacrificial-fire prize;
- the Ghost, Wandmaker, Blacksmith and Imp reward options;
- the Imp's depth-20 shop cache, when `--floors` reaches the boss level.

Reading `trueLevel()` (rather than `name()` or `identify()`) is what keeps the
scan from mutating a generated item, so the search cannot perturb the run it is
searching. A seed's floors stop once the complete query matches. The compound benchmark
respects the vault's single-item limit, including the Imp reward options.

## Effective-match protocol

The Python driver in `tooling/benchmarks/effective_matches.py` starts persistent
JVMs with `--stream`. Each prints `{"ready":true}` after untimed warmup, then
accepts one JSON object per input line:

```json
{"seeds":[3901899992008]}
{"seeds":[3901899992008],"verify":true,"trinkets":["ParchmentScrap"]}
```

The first request searches the no-trinket world and stops when the query matches.
The second generates through floor 24 and the Imp's Vault, returning every
matching item **within the requested query depth**, including an empty witness
list for unsuccessful seeds. Each trinket choice must be among the four initial
offers. A chosen trinket is inserted at +3 after the first floor by whose end
both a catalyst and an alchemy opportunity have appeared, matching Seed Seeker's
existing generation profile. This validates that profile; it does not simulate
collecting the energy needed for +3 or arbitrary gameplay actions.

Responses contain `tested`, internal `seconds`, and `matches`. Every match has
a numeric `seed` and `witnesses`: `[depth, source, stable_item_id, upgrade,
cursed, enchantment_or_glyph]` tuples (`"-"` means no effect). Quest effects are
read from the quest's deferred enchantment/glyph fields. Vault reward choices
are recorded as Imp rewards, separately from vault treasure. The driver records
request-to-response wall time, so protocol overhead is included in its metric.

The adapter supports single-item queries and the benchmark's +2 Runic Blade
and +2 Ring of Might query. Other compound queries are rejected.

## Headless technique

Startup is the oracle's, and the oracle's README explains it in full:
`Game.version` is set to a `-INDEV` string so `DeviceCompat.isDebug()` marks the
journal read except for the Halls `attrition` page, `GameSettings.set(new MemoryPreferences())` provides in-memory
settings, `Badges.global` and the `Bones` fields are reset, and the geometry-only
`com.watabou.noosa.TextureFilm` stand-in keeps `ItemSpriteSheet.Icons.film` off
the GPU. That stand-in is compiled straight out of `tooling/oracle-4.0/src`
rather than copied, so the two tools cannot drift apart.

Two things are specific to searching many seeds in one process:

1. **A second stand-in, `ItemSprite`.** Generation is otherwise sprite-free,
   but `Level.drop()` builds a throwaway `Heap` with a `new ItemSprite()` when
   the item it is given is null, and `RingRoom.placeCenterDetail` gives it
   `Level.findPrizeItem()`, which is null once a floor's prize items are spent.
   The upstream constructor needs a GL context, so roughly one seed in a hundred
   dies inside nineteen floors. `src/.../sprites/ItemSprite.java` replaces the
   class with a drawing-free one: it still extends `MovieClip`, but its
   constructors stop at `MovieClip()`, every method is a no-op, and it touches
   `com.watabou.utils.Random` nowhere, so the seeded stream is unchanged. The
   heap that path builds holds no item, so nothing searchable is lost.
   **`tooling/oracle-4.0` has the same gap** and crashes on those seeds
   (`AAA-AAA-ADU` is one); the stand-in is kept here rather than shared until
   the oracle's fixtures are re-run with it.
2. **Run statics the game never has to reset.** `Dungeon.init()` resets the
   Generator, the limited drops and each quest, but `Imp.Quest.reset()` leaves
   `rewardOptions`, `oldQuest` and `alternative` alone — upstream clears the
   reward options in `VaultFinalRoom.paint()` and never reuses a process. In a
   seed loop a run whose vault is never built would hand its reward options to
   the next seed, which then reports them as its own. `JarSeedFinder` restores
   all three to the values a fresh JVM holds before every seed.

Without the second fix the finder reports steadily more matches the longer it
runs; the cross-check below is what makes such a leak visible.

## Threads

The game's generator state is global (`Dungeon`, `Generator`, `Random`,
the quest classes), so one JVM searches one seed at a time. A multi-core figure
comes from several processes over disjoint ranges, the way the incumbent's own
`tools/turbo.js` drives it:

```sh
for i in $(seq 0 5); do
    tooling/java-finder/run.sh --start $((i * 100000)) --seeds 2000 &
done
wait
```

## Generation shortcuts

Seed Seeker plans a query before searching, and for the +5 Crossbow control the plan
takes two shortcuts. Both are enabled for its Java baseline:

- `--no-vault`. A `+5` only ever appears on a tier-4 weapon in
  `Imp.Quest.rewardOptions`, which is rolled on the Imp's City floor; the
  vault's own treasure stops at `+4`. The engine builds the sub-level only when
  a requirement could be met by vault treasure, so for this query it does not,
  and neither should the baseline. The reward options are read on the Imp's
  floor either way.
- `--skip-boss-floors`. Depths 5, 10, 15 and 25 leave the Generator, the limited
  drops, the quests and the room queues untouched — `tooling/oracle-4.0`'s
  `boss-skips.sh` pins that for three seeds — and each floor is seeded
  independently by `Dungeon.seedForDepth`, so stepping over them changes no
  later floor. The engine never simulates them. Depth 20 is *not* neutral (it
  caches the Imp's shop) and is never skipped.

## Cross-check

The [README benchmark](../../README.md#benchmarks) checks the complete Java
sample, including negative seeds, against the native engine. It also replays
every returned recipe through floor 24 and checks supporting item details in
the JAR, outside timing. A regression seed rejects taking both an Imp ring and
a blade from the vault. Raw evidence is written to the requested output
directory and is not committed.

## Isolation

No file outside this directory is written; `.work/` (the JAR and the compiled
classes) is gitignored. Shattered Pixel Dungeon is GPL-3.0 software, and the
finder and its stand-in are provided under the same license.
