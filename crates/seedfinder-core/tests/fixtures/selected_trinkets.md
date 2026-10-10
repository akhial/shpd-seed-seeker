# Selected trinket regression fixtures

`selected_trinkets_v4.0.2.json` contains 21 official-JAR equipment cases across
seven trinkets, each equipped at +3 after the first brewing opportunity.
Every case was regenerated for 4.0.2 with Eclipse Adoptium JDK 25.0.4.1.
Loot includes the main floors through 24 and the Imp's vault, including Rat
Skull statue equipment and Mossy Clump feeling rolls.

The official JAR is pinned by `tooling/oracle-4.0/build.sh` (SHA-256
`bcefd52a9c14f69c69d3ccc08a212cdd3830b2dc60d78d80895cd803fa1ec602`),
matching source commit `57a4e06a4caf162446d1c28caa7983f0493fecf0`.
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
