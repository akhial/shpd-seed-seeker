# Daily-run scouting

The apps and CLI scout the daily run for a selected UTC calendar date. The
date picker accepts past and future dates; **Today** reads the UTC date when
pressed, so it remains correct across midnight and in every device time zone.

The pinned game's [`Dungeon.initSeed`](https://github.com/00-Evan/shattered-pixel-dungeon/blob/2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/Dungeon.java#L206-L212)
computes `UTC midnight Unix milliseconds + 5_429_503_678_976`. This value is
outside the normal seed-code range. Converting it to nine letters, reducing it
modulo `26^9`, or entering the date as a custom text seed produces a different
run. For example, `2026-09-25` has the raw seed `7_219_798_078_976`.

All dates use the app's pinned generation version, currently Shattered Pixel
Dungeon 4.0.0. The game itself permits 4.0 dailies starting on April 1, 2026.
Earlier dates are extrapolations with this engine, not replays of older game
versions. Future dates likewise assume this game version.

## Shared contract

- `DungeonSeed::from_daily_date` validates a strict Gregorian `YYYY-MM-DD`
  between `1970-01-01` and `9999-12-31`; `from_scout_input` accepts either that
  date or a normal seed code. `new` and `from_code` retain the original
  searchable seed range.
- The existing seed parsing bridges return `{ "code": "2026-09-25", "value":
  7219798078976 }` for a date. The interactive formatter preserves complete,
  valid dates supplied by pickers and links. Native `isCanonical` checks still
  refer to nine-letter codes; `isScoutable` additionally accepts daily dates.
- Native scout request seed fields (`SSQ2` through `SSQ5`, and legacy text)
  and WASM's `scout({seed, ...})` accept dates. Scout response identities keep
  the date, including trinket overrides, query match marks, and item mappings.
  Existing length-prefixed packet fields need no new wire version.
- Level-map JSON requests accept the same date in `seed`, with the same
  challenges and trinket options. The returned map retains that date.
- The web also accepts `?scout=2026-09-25`. Daily seeds do not become searchable
  or importable as ordinary search results.

## Verification

The calendar tests cover Gregorian leap years and centuries, invalid dates,
range endpoints, and both sides of midnight UTC. The official JAR oracle
supports `--daily` to exercise the real `SPDSettings.lastDaily` and
`Dungeon.initSeed` path:

```sh
tooling/oracle-4.0/run.sh --daily 2026-09-25 --floors 1 --format json
cargo test -p shpd-seedfinder-core --test daily_runs
```

The regression fixtures include its raw seed, first-floor RNG seed, scroll
rune, and all ring gems. Native bridge and browser tests exercise scouting,
date identity, and switching trinkets and maps.
