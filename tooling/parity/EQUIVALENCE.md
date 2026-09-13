# Exact v4.0.0 item and cell equivalence

This harness generates every supported floor through 24 and the Imp vault for
consecutive seeds with zero challenges and a Warrior. It compares sorted
multisets of `(floor, source, item, upgrade, cursed, effect)` without hashes or
search early exits. Duplicate entries count separately. All catalog weapons,
armor, rings, wands, and artifacts are included, including quest choices, shops, mimics,
and statues, plus the four initial catalyst offers. A separate sorted multiset
compares `(floor, branch, cell, item, upgrade, cursed, effect)` for physical
equipment and artifacts in heaps, shops, mimics, and statues. Quest reward choices and the
unplaced floor-20 shop cache have no physical cell in this profile.

For all 20 regular floors and the vault, every integer terrain cell is compared
directly, along with width and height. No map hashes substitute for cell equality.
The production world generator supplies the item multiset; the regional floor
generators expose the terrain and placement event records used by the cell check.
Consumables, quantities, mob identities, custom visual tiles, secret flags, and
accessibility groups are outside this comparison. Boss terrain is not simulated
by the engine. These boundaries are recorded in each run's manifest.

The oracle loads the official 4.0.0 JAR, build 912 (SHA-256
`b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1`).
Only the existing headless `TextureFilm` and `ItemSprite` stand-ins precede the
JAR; no generation classes are substituted. The one-seed finder startup is
outside the requested interval. Each subsequent seed resets the game and the
Imp's persistent reward fields. Boss floors 5, 10, and 15 contribute no catalog
equipment or persistent item-deck changes and are skipped; floor 20's shop is
included. The vault is generated after the main dungeon, as in the engine.

The [v4.0.0 validation report](v4.0.0-10000.json) records the final-release
source comparison, the eight oracle fixture groups, and a fresh exact
10,000-seed comparison through floor 24 plus the vault. The source commit is
`2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`. The published history confirms
that seeded generation is unchanged from the preceding release candidate:
only version metadata, translations, changelog, and visual/UI code changed.

The oracle fixtures now include the artifacts its item recorder already
marks searchable. Their prior equipment, maps, and generator checkpoints
are unchanged. The batch oracle also includes artifacts, using the Imp's
transfer amount (+5) for search records and internal levels for physical cells.

The [archived prerelease report](prerelease-100000.json) retains its original
version, executable hashes and evidence paths. Its 100,000-seed run covered
equipment and terrain before artifacts were added to the comparison; it is
historical evidence, not a validation run of the final release.

## Run

From the repository root, build the existing Java finder and Rust comparator:

```sh
bash tooling/java-finder/build.sh
cargo build --release -p shpd-seedfinder-core --example equivalence
```

Compile `tooling/parity/BatchEquipmentOracle.java` with the selected JDK into
`tooling/oracle-4.0/.work/batch-classes`, using these classpath entries:

- `tooling/java-finder/.work/classes`
- `tooling/oracle-4.0/.work/ShatteredPD-v4.0.0-Java.jar`

Use `;` between classpath entries on Windows and `:` elsewhere. Run:

```sh
python tooling/parity/run_equivalence.py 100000 6 \
  --output tooling/oracle-4.0/.work/equivalence-new \
  --exe target/release/examples/equivalence
```

On Windows use the `.exe` suffix and the GNU Rust toolchain if MSVC is absent.
Set `JAVA_21_HOME`, `JAVA_HOME`, or pass `--java` with the Java executable.
The final-release validation used Eclipse Adoptium JDK 25.0.4.1 on macOS arm64.
The output directory must not exist. The runner freezes the comparator binary
and records its hash, the Git revision, oracle hash, and exact seed coverage.

Each shard retains compressed oracle records, JSON multiset differences,
stderr, and a completion record. `progress.json` is partial progress only.
A successful result requires every shard to finish, each comparator summary
to report the requested count with zero deviations and errors, and all process
exit codes to be zero. The comparator rejects missing or out-of-order seeds.
Generating full dungeons is much slower than the finder search benchmark;
measure sustained throughput before estimating a multi-million-seed run.

## Recheck a fix without regenerating the oracle

```sh
python tooling/parity/replay_equivalence.py \
  tooling/oracle-4.0/.work/equivalence-new \
  /absolute/path/to/frozen-fixed-equivalence
```

An optional third argument sets a new evidence label (default `fixed`) for
another replay of the same archives without overwriting prior results.

This follows an active archive or reads a completed one and writes separate
`fixed-*` evidence. Preserve the supplied binary while replay is running.
The gzip end marker and exact seed counts are checked before completion. Both
runner and replay return a nonzero exit code for any failed shard.

The v4.0.0 records contain four pipe-separated fields: numeric seed, item multiset,
full maps, and physical equipment multiset. This comparator rejects the older
equipment-only streams rather than treating missing cell coverage as a pass.

To exercise the comparator's negative controls against a completed shard:

```sh
python tooling/parity/test_equivalence.py --exe target/release/examples/equivalence \
  --oracle-archive tooling/oracle-4.0/.work/equivalence-new/shard-0.oracle.txt.gz
```

This checks an exact record, changed upgrades, duplicate items, changed terrain,
moved equipment, missing floors, missing/duplicate seeds, and oracle errors.

The v4.0.0 release leaves the Halls King's `attrition` page unread in debug defaults. Its
floor-24 placement clears tall grass even though it is not searchable loot.
Seed 17 / `AAA-AAA-AAR`, cell 1191, is the engine regression for this case.
The Segmented Library's successful horizontal-split return and both no-trinket
feeling rolls were already present in the Rust port and match v4.0.0.

The checked-in `beta4-mob-retry-equipment.txt` fixture records three retry failures
from the large run (667551, 1334551, 1334612). They exposed Sewer mob placement
accepting the 31st candidate after Java's retry budget expired, changing later
item draws. The quest-room fixture also checks Mass Grave loot's Skeleton source.

Seed 905410 additionally covers disconnected adjacent Platform Rooms in the
Caves. Their shared wall must preserve the requested solid decorative terrain;
only connected Platform Rooms substitute chasm and restore the doorway. The
wrong wall changes entrance-distance exclusions, mob placement, and later loot.
