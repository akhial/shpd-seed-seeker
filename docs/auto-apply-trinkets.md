# Automatically choosing one trinket

The **Search scope → AutoTrinket** option in web, Android, Linux, macOS and Windows is on by default for new queries
and built-in presets. Saved queries, shared links and imports retain their
recorded setting, including the legacy off default when the flag is absent.
It asks the engine to select a helpful trinket from the seed's four initial catalyst offers
before generating any floors. Each seed gets one initial search. Only a match
with an automatic trinket gets a second, no-trinket generation pass. If it still
satisfies the full query, the engine returns that no-trinket world and recipe;
otherwise it retains the necessary trinket. Failed searches are not retried,
and items from different possible worlds are never combined. The existing
+3 model activates the choice at the first brewing opportunity; it does not
change equipment on floors generated before then.

The engine ranks Parchment Scrap, Mimic Tooth, Rat Skull and Cracked Spyglass
using the existing conditional equipment probability profiles for the whole
query. A candidate is preferred when its estimate exceeds baseline by 5%.
This is a conservative fixed threshold, not a promise that the candidate
improves every seed. Parchment Scrap is excluded whenever any requested effect
is a curse, including inside alternatives.

For each offer deck, the engine chooses the highest-ranked preferred offer.
If no preferred trinket is offered, it generates the no-trinket world. Neutral
trinkets (including Exotic Crystals), candidates without the required estimated
benefit, Mossy Clump, and Trap Mechanism are never selected automatically.
Any explicit trinket requirement disables auto-apply, whether selected or merely
required, and whether standalone or in an OR group.

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
with `SeedRecipe` choices, stripping unnecessary automatic choices only after
rechecking the full query with the same floor and vault pruning. Explicit
trinket requirements are not stripped. WASM only adapts those operations to cooperative
sessions and JSON; there is no selection or generation algorithm in the web.
Native UIs use the same policy and match cleanup through `NativeSession`.
GTK consumes typed recipes; JNI and C FFI deliver `SSR2` packets with an explicit
trinket ID or no-trinket marker. Saved recipes and their base query also travel
through native refinement. Native clients use platform switches in Search scope.

The query JSON codec persists `auto_apply_trinket` only when true. Share links
use version 6 for the flag; existing version 4 and 5 queries retain their
byte format and remain readable. Results stay on one line with a small,
translucent trinket sprite beside the seed only when a trinket is applied.
The name is available on hover and to screen readers. Results export the
exact choice, and scout using that choice and the saved query rather than
the current editor. See [results export](results-export-format.md).

Scout keeps the four initial offers available while browsing later floors.
As the large offer cards scroll behind the top edge or sticky floor header, small sprite
buttons slide into the navigation bar while its keyboard/swipe hint fades.
They retain the offer order and highlight the selected trinket without a text
badge. Switching trinkets preserves the current floor and scroll offset;
clicking the selected trinket again scouts without it.

Continuation compares the prepared choice policies. A different policy
requires a new traversal; matching policies can reuse coverage and filter
saved recipes. `refine_batch` also retries a previously stripped choice when
the no-trinket recipe fails a changed query: a trinket unnecessary for the old
requirements may now be necessary. Unchanged queries keep their saved choices,
subject to the same unnecessary-trinket check. See [search semantics](search-semantics.md).

## Validation

Core regression tests enumerate all four-offer subsets, verify the no-trinket
fallback against baseline worlds, check curse and
manual-requirement exclusions, count generation inputs, and verify continuation
rules, and check that only auto-applied matches get a baseline replay. Seed
`EYY-RUL-LQG` needs no trinket for a +1 Grim Runic Blade, but refining that query
to also require a Venomous Whip restores Parchment Scrap. The known
selection-only result `SRU-YSU-QHS` matches a +1 Grim Runic
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
Browser checks also covered the compact trinket controls' reveal and reverse
animation, reduced motion, offer order after changing seeds, and switching or
clearing a trinket on floor 14 while retaining the same scroll offset.

Native regression tests cover the setting in presets, saved queries and shared
links, recipe packets, explicit no-trinket exports, and the same necessary/stripped
seed fixtures through JNI, C FFI and the Swift engine adapter. Match selection,
reverification and recovery remain in Rust; the frontends only render the recipe.

GTK's display test exercises the partial hint fade, complete reveal, changing
from Mimic Tooth to Parchment Scrap, clearing selection, and preserving the
visible floor and offset after item layouts change. Run it on a GTK-capable host
with `xvfb-run -a dbus-run-session -- cargo test -p shpd-seedfinder-gtk scout_dock_tracks -- --ignored`.
