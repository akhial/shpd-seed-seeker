# Artifact search and scout

The engine searches the 11 artifacts in the deterministic spawn deck. A
requirement must name an artifact: `kind: "artifact"` alone is rejected, just
like a wildcard trinket. Cloak of Shadows and Holy Tome have zero spawn weight
and are not offered in the artifact picker.

```json
{
  "requirements": [
    { "item": "ethereal_chains", "max_depth": 9 },
    { "item": "sandals_of_nature", "source": "imp_reward", "upgrade": 5 }
  ],
  "max_depth": 19
}
```

Artifacts support named AND/OR queries, individual floor limits, source and
uncursed filters. Artifact editors use any upgrade and do not offer an upgrade
selector; scout results still show the generated upgrade. The generator's existing unique artifact deck is unchanged.
Only artifacts generated at deterministic locations participate: heaps,
containers, skeletons, shops, pre-generated mimic contents and the Imp reward.
Runtime drops, purchases affecting later RNG, and other player actions that could
change the deck are not simulated, except for the explicit transmutation search
described below.

Ordinary artifacts are +0. The Imp quest's Dwarven vault artifact receives
`transferUpgrade(5)`. Scout badges match the game's rounded displayed level:
**+7** for Sandals of Nature (internal level 2), **+6** for Ethereal Chains and
Timekeeper's Hourglass (internal level 3), and **+5** for the other artifacts.
Both the transfer and display conversions round to the nearest integer, with
halves rounding up. Raw wire/search upgrades retain the transfer amount for
compatibility with existing queries and probability tables. That reward
is uncursed and shares the vault's single-pick accessibility group with all
other Imp rewards and vault treasures. Matching cannot acquire it together
with another option from that group.

Artifact records carry the normal source, floor, curse, secret and accessibility
metadata. Core `SSC3`/`SSC4`/`SSC5` wire records use stable artifact IDs with the existing
upgrade field. Query JSON and result files use `artifact`; share links append
category code 7 and artifact item codes, preserving previous codes. WASM scout
JSON reports `category: "artifact"` and the normal `matched` flag.

Web, Android, Windows, macOS, and Linux use the shared item catalog for artifact
pickers and scout results. Native scout packets and match indices include the
same full artifact stream as the core generator and WASM.

Artifact probability estimates use deterministic supply measured over 200,000
BETA-4 worlds, including floor, source, curse, and upgrade distributions.
Single-item estimates use the measured supply; joint artifact estimates average
all valid assignments of their unique identities over 4,096 anonymous generated
layouts. This respects the finite deck and each layout's accessibility choices.
The layouts are regenerated with `cargo run --release --example calibrate_artifacts`
into `src/probability_tables/artifact_worlds.bin` within the core crate.
Artifacts compete with equipment for the Imp vault prize. Like equipment estimates, mixed supply and alternative groups retain
the estimator's documented approximations.

Tests pin all 11 artifact identities across eight worlds against the official
BETA-4 JAR and cover
floor limits, gated/scalar search, vault exclusivity, codecs, and the web scout.
The parity oracle emits `search_upgrade` alongside `true_level` so transferred
artifact levels can be compared without losing the underlying Java level.

## Artifact transmutations

Enable **Allow transmutations** on a named artifact and choose a maximum of
1–10. Natural finds still match. JSON uses `artifact_transmutations` (default 0):

```json
{"requirements":[{"item":"ethereal_chains","artifact_transmutations":4,"max_depth":19}]}
```

The search uses the remaining artifact deck **after generating the requirement's
floor limit**, bounded by the overall query limit. It needs an obtainable starting
artifact by that floor. Source, curse, and accessibility constraints apply to that
starting artifact; the target must be in the allowed prefix of the remaining deck.
Upgrades transfer through each intermediate artifact using the game's displayed
level conversion, assuming an upgraded starting artifact has been identified.
One starting artifact cannot satisfy two kept-item requirements, and unique target
identities cannot be duplicated. Multiple targets at the same floor share the deck.

Snapshots describe the unmodified generated run. The matcher does not combine
transmutations at different floor limits, or retain an artifact generated after an
earlier transmutation, because those would require replaying later generation.
Scroll availability and effects on later generation are not simulated. Exhausting
the deck produces a ring, which is not an artifact match.

Transmutation probability uses a separate joint donor/deck model measured over
8,192 worlds for each of the eight generation profiles. It averages identity
assignments without replacement, preserves obtainable donor combinations and
floor boundaries, and reserves the Imp prize across artifacts and equipment.
AutoTrinket uses the same calibrated scores to select and estimate its policy.
No worlds are generated while estimating. See [calibration and timing results](probability-artifact-calibration.md).
Imported queries with upgrade-filtered artifact transmutations remain unavailable:
rounding through intermediate identities is not calibrated. The normal artifact
editors use any upgrade. Extremely complex joint filters also have a bounded work
budget and report unavailable instead of blocking the editor.

Scout shows **Artifact transmutation order** with an **After floor** selector on
web, Android, macOS, Windows, and Linux. Positions start at 1; matching outcomes
and their starting artifacts are highlighted. Boss floors inherit the last generated
deck. The CLI displays the deck at the query's overall floor limit (24 without a
query). Reading a deck never advances the generated run's RNG.

Share-link version 14 adds the four-bit artifact limit after version 13's trinket
limit; older links retain their exact encoding. Native clients request `SSQ6` and
receive `SSC9`: the `SSC8` body followed by a one-byte snapshot count, then each
snapshot's one-byte floor, one-byte identity count, and stable artifact IDs encoded
as big-endian u16-length-prefixed UTF-8 strings. Older scout requests retain
legacy packet layouts. Match JSON adds `transmutedArtifacts` entries with `depth`
and zero-based `index`; WASM exposes all floor decks as `artifactDecks`.

The official v4.0.0 oracle in `tooling/oracle-4.0/ArtifactOracle` drains the saved
artifact deck after a floor prefix and optionally runs actual scroll transmutations.
Pinned remaining-deck fixtures supplement the existing generated-artifact parity
checks. The implementation follows the upstream
[artifact transmutation code](https://github.com/00-Evan/shattered-pixel-dungeon/blob/v4.0.0/core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/items/scrolls/ScrollOfTransmutation.java).
