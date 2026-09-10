# Automatically choosing one trinket

The web's **Performance → Auto-apply a trinket at +3** option is off by default.
It asks the engine to select one of the seed's four initial catalyst offers
before generating any floors. Each seed is generated once, with no baseline
scout, retry, or union of loot from different possible worlds. The existing
+3 model activates the choice at the first brewing opportunity; it does not
change equipment on floors generated before then.

The engine ranks Parchment Scrap, Mimic Tooth, Rat Skull and Cracked Spyglass
using the existing conditional equipment probability profiles for the whole
query. A candidate is preferred when its estimate exceeds baseline by 5%.
This is a conservative fixed threshold, not a promise that the candidate
improves every seed. Parchment Scrap is excluded whenever any requested effect
is a curse, including inside alternatives.

For each offer deck, the engine chooses the highest-ranked preferred offer.
If none is offered, it chooses the first neutral offer. Exotic Crystals is
neutral for the current searchable equipment catalog: its consumable
conversions do not change the generation RNG draws used by this model. If no
neutral choice is offered, it chooses the highest-ranked allowed candidate.
Mossy Clump and Trap Mechanism are excluded even from this last resort.
There is always an allowed choice among four offers. Any explicit trinket
requirement disables auto-apply, whether selected or merely required, and
whether standalone or in an OR group.

The choice is an estimate based on the query and offer deck, not advance
knowledge of a seed's contents. Some baseline matches are lost; the purpose
is to improve the expected rate of finding a match across seeds. The
[prototype study](single-trinket-study.md) and the
[production benchmark](auto-trinket-production-benchmark.md) quantify that
tradeoff. Common queries may gain matches per second without showing the
first result earlier because the web delivers batches of 256 seeds.

## Engine and frontend responsibilities

`auto_trinkets::AutoTrinketPolicy` prepares the ranking once per `QueryPlan`.
`FloorGate::selected_trinket` supplies the choice to the existing production
generator. Shared `search_batch` and `filter_batch` return matching worlds
with `SeedRecipe` choices. WASM only adapts those operations to cooperative
sessions and JSON; there is no selection or generation algorithm in the web.
Native engine callers also receive selection through `QueryPlan`. Remaining
UIs have no new toggle or result-recipe integration in this change.

The query JSON codec persists `auto_apply_trinket` only when true. Share links
use version 6 for the flag; existing version 4 and 5 queries retain their
byte format and remain readable. Results show “Choose … +3,” export the
exact choice, and scout using that choice and the saved query rather than
the current editor. See [results export](results-export-format.md).

Continuation compares the prepared choice policies. A different policy
requires a new traversal; matching policies can reuse coverage and filter
saved recipes. See [search semantics](search-semantics.md).

## Validation

Core regression tests enumerate all four-offer subsets, check curse and
manual-requirement exclusions, count generation inputs, and verify continuation
rules. The known selection-only result `SRU-YSU-QHS` matches a +1 Grim Runic
Blade through floor 19 with Parchment Scrap, fails without it, and reproduces
through a complete floor-24 scout. The early floors remain unchanged.

Web tests exercise the real WASM session, filter and scout paths, query/share
serialization, exported choices, duplicate import handling, single-core
settings, and explicit-requirement disabling. Browser checks against the
hosted production build additionally covered toggle persistence, a live
single-worker search, cancellation, matching result scouting, and export.
An imported Parchment result still scouted and exported its original recipe
after auto-apply was switched off in the editor. Desktop and 390-pixel mobile
layouts were checked with no page errors or horizontal overflow.
