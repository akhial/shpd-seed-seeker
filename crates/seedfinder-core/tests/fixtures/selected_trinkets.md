# Selected trinket regression fixtures

`selected_trinkets_v4.0.1.json` contains 21 official-JAR equipment cases across
seven trinkets, each equipped at +3 after the first brewing opportunity.
Every case was regenerated for 4.0.1 with Eclipse Adoptium JDK 25.0.4.1.
Loot includes the main floors through 24 and the Imp's vault, including Rat
Skull statue equipment and Mossy Clump feeling rolls.

The official JAR is pinned by `tooling/oracle-4.0/build.sh` (SHA-256
`452a4b3811d271ff6078ed352905516d3a48aeac310b4b6609cbb7f44bf534ea`),
matching source commit `e9defd0444c96d2fce3de5ec297c3398be8b7c55`.
After building the oracle, regenerate all cases with:

```sh
python3 tooling/oracle-4.0/generate-selected-trinkets.py
```

The oracle receives `-Dseedfinder.trinket=<JavaClass>` through `JAVA_TOOL_OPTIONS`.
The generator compares equipment as sorted multisets of depth, stable item ID,
search upgrade, curse status and enchantment/glyph. It excludes artifacts, plain
darts and `imp_quest` records (the vault already contains those rewards), and
preserves duplicates. The official debug journal leaves the Halls `attrition`
page unfound, matching the engine's canonical profile.
