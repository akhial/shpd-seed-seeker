# Mining map fixtures

`mining-maps.expected.json` records the complete initial map's Java array hash,
size, entrance, secret-room bounds and trap cells. It covers three seeds,
depths 12–14, both Blacksmith variants, and challenge masks 0/104 (36 maps).

These mining fixtures use the **official v4.0.0 release**, matching the
generation engine's pin. The recorded JAR URL and SHA-256 are in the JSON. No game
classes are changed: `MiningMapOracle` invokes the existing headless bootstrap,
forces the requested Blacksmith type, and calls `Dungeon.newLevel()` at branch
1. There are no bones or equipped trinkets. Map API tests separately validate
the quest's actual location and variant in a run with the selected trinket.

To reproduce, download and verify the JSON's JAR, then from the repository root:

```sh
javac -encoding UTF-8 -cp /path/to/ShatteredPD-v4.0.0-Java.jar -d /tmp/mining-oracle \
  tooling/oracle-4.0/src/com/watabou/noosa/TextureFilm.java \
  tooling/oracle-4.0/src/com/shatteredpixel/shatteredpixeldungeon/ParityOracle.java \
  tooling/oracle-4.0/src/com/shatteredpixel/shatteredpixeldungeon/MiningMapOracle.java
java -cp /tmp/mining-oracle:/path/to/ShatteredPD-v4.0.0-Java.jar \
  com.shatteredpixel.shatteredpixeldungeon.MiningMapOracle AAA-AAA-AAA 12 1 0
```

Arguments are seed, depth, variant (`1` Crystal / `2` Gnoll), challenge mask.
Repeat for `AAA-AAA-AAA`, `AAA-AAA-AAB`, `AAA-AAA-AAF`; depths 12, 13, 14;
variants 1, 2; masks 0, 104. The headless bootstrap's debug journal setup is
inherited from `ParityOracle`; it does not add any items to the mine.

Rust validation: `cargo test -p shpd-seedfinder-core --lib mining_floor::tests`.
