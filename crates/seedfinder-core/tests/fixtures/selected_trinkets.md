# Selected trinket regression fixtures

`selected_trinkets_v4.0.0.json` retains 21 equipment regression cases across
seven trinkets, each equipped at +3 after the first brewing opportunity.
Loot includes the main floors through 24 and the Imp's vault. The
`AAA-AAA-AAF` / `cracked_spyglass` case was rechecked without changes against
the final v4.0.0 release with Eclipse Adoptium JDK 25.0.4.1.

The official JAR is pinned by `tooling/oracle-4.0/build.sh` (SHA-256
`b3e6f9508dea1a7a32a9934e2bc18f20a9a905df5732550404294340d31c87a1`),
matching source commit `2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f`.
After building that oracle, run:

```sh
JAVA_TOOL_OPTIONS=-Dseedfinder.trinket=CrackedSpyglass \
  tooling/oracle-4.0/run.sh --seed AAA-AAA-AAF --floors 1-24 --vault --format json
```

Compare equipment item records as sorted multisets of depth, stable item ID,
search upgrade, curse status and enchantment/glyph. Exclude artifacts, plain
darts and `imp_quest` records (the vault already contains those rewards).
Preserve duplicates. The v4.0.0 debug journal leaves the Halls `attrition` page
unfound; forcibly marking it read reproduces the obsolete floor-24 Greataxe
and Wand of Corruption. With the final release defaults those two drops are
absent.

The other cases retain their BETA-4 provenance. Full regeneration also
exposes Mossy Clump profile differences; these historical regression cases
are not evidence of full selected-trinket parity with the final release.
