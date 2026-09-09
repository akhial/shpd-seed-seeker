# Selected trinket regression fixtures

`selected_trinkets_rc1.json` retains the 21 BETA-4 equipment regression cases
and updates `AAA-AAA-AAF` / `cracked_spyglass` for RC1. The seven trinkets are
each equipped at +3 after the first brewing opportunity; loot includes the
main floors through 24 and the Imp's vault.

The updated case was captured with Eclipse Adoptium JDK 25.0.4.1 against the
official RC1 JAR pinned by `tooling/oracle-4.0/build.sh` (SHA-256
`43f881f0d6484faffea913f5563fd2c3277ed83159eda6e83efc55e586fbfdbf`).
After building that oracle, run:

```sh
JAVA_TOOL_OPTIONS=-Dseedfinder.trinket=CrackedSpyglass \
  tooling/oracle-4.0/run.sh --seed AAA-AAA-AAF --floors 1-24 --vault --format json
```

Compare equipment item records as sorted multisets of depth, stable item ID,
search upgrade, curse status and enchantment/glyph. Exclude artifacts, plain
darts and `imp_quest` records (the vault already contains those rewards).
Preserve duplicates. RC1's debug journal leaves the Halls `attrition` page
unfound; forcibly marking it read reproduces the obsolete floor-24 Greataxe
and Wand of Corruption. With RC1 defaults those two drops are absent, and
every other equipment entry in this case is unchanged.

The other cases remain historical regression fixtures. Full regeneration
also exposes Mossy Clump profile differences, which are separate from the
RC1 Cracked Spyglass CI failure; this update does not replace those cases.
