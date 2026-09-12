# Raised map rendering fixtures

`raised-map-visuals.expected.json` records all-cell atlas-index hashes from the
unmodified official **v4.0.0** desktop JAR, matching the generation engine's pin.
JAR SHA-256:

```
b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1
```

The 13 maps cover depths 1, 7, 12, 16, 23 for AAA-AAA-AAA and JHG-HJJ-BKK,
plus AAA-AAA-AAA's Crystal mine (13/1), JHG-HJJ-BKK's Gnoll mine (12/1), and
AAA-AAA-AAA's Imp vault (19/1). Each record hashes the input terrain and five
independently selected rendering layers: terrain, occlusion shadows, features,
raised terrain, and upper walls. An absent sprite is -1; transparent shadow tile
0 is normalized to -1. Hashes use Java `Arrays.hashCode(int[])`.

The oracle receives canonical terrain from the Rust endpoint, so this checks
rendering selection, not map generation. It bypasses only GL texture allocation
using `Unsafe.allocateInstance`, then calls the original protected selectors.
There are no substitutes for the game's rendering algorithms on the classpath.
Plants/traps are empty in this selector comparison; their independent atlas
indices and visibility are covered by engine tests. The permanent geometric
black masks and secret-room concealment also have separate engine tests. Dynamic
lighting, live entities, custom room decorations and frame-time particle motion
are outside these selector fixtures.

Reproduce from the repository root, supplying the official release JAR:

```sh
cargo build -p shpd-seedfinder-core --features json-query --example level_map
javac -cp /path/to/ShatteredPD-v4.0.0-Java.jar -d /tmp/map-oracle \
  tooling/oracle-4.0/src/com/shatteredpixel/shatteredpixeldungeon/MapVisualOracle.java
target/debug/examples/level_map \
  '{"seed":"JHG-HJJ-BKK","depth":7}' > /tmp/map-input.json
java -cp /tmp/map-oracle:/path/to/ShatteredPD-v4.0.0-Java.jar \
  com.shatteredpixel.shatteredpixeldungeon.MapVisualOracle /tmp/map-input.json
```

Repeat for the fixture requests above, adding `"branch":1` for quest maps. An
optional second oracle argument is a file prefix to dump each layer's complete
index array for diagnosing a mismatch. No oracle runtime is needed for Rust CI.
