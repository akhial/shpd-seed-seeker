# Auto-apply trinkets during search

The shared Rust engine exposes `auto_trinkets::search_batch`. The web app is
the first frontend to opt into it, through **Performance → Auto-apply
trinkets**, next to Workers. The preference is persisted locally and defaults
to off. It is inactive whenever any requirement, including an OR alternative,
names a trinket. Explicit trinket selection keeps its existing behavior.

For each seed, the engine checks the baseline first. If it does not match,
auto-apply tries each applicable trinket in the seed's four initial catalyst
offers independently. Each branch regenerates from the original seed at +3
potency, using the existing activation rule: effects begin on subsequent
floors after both catalyst and alchemy availability. No effect is applied
retroactively, and branches never combine items or trinkets. Branches use the
same floor pruning, query depth, challenges, quest conditions, and acquisition
constraints as normal searches.

The first matching branch supplies one result and its trinket recipe. A seed
counts once toward progress and result limits. Baseline matches take
precedence and remain in the result set; later branches need not be generated
once a seed matches.

## Included trinkets

- Parchment Scrap
- Mimic Tooth
- Rat Skull
- Cracked Spyglass
- Mossy Clump
- Trap Mechanism

Mossy Clump and Trap Mechanism change later generation sufficiently to create
searchable matches, even though their direct effects concern floor feelings.
The core regression tests pin two baseline misses:

| Trinket | Seed | Query |
| --- | --- | --- |
| Mossy Clump | `AAA-AAA-AAC` | Uncursed Cleansing Dart by floor 6 |
| Trap Mechanism | `AAA-AAA-AAF` | +1 Explosive Spear by floor 7 |

Exotic Crystals is excluded. The current searchable catalog does not include
potions or scrolls, and their exotic-conversion RNG draws are consumed with
or without the trinket (`generator.rs` and `special_consumable.rs`). The
[benchmark study](auto-trinkets-benchmark.md) records the additional seed
comparison used to check for indirect gains.

## Core API

```rust
use shpd_seedfinder_core::{
    auto_trinkets::search_batch,
    feasibility::QueryPlan,
    main_world::CanonicalMainWorldGenerator,
};

// query is validated; seeds is a slice of DungeonSeed values.
let plan = QueryPlan::analyze(&query);
let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
let matches = search_batch(&generator, &query, &plan, &seeds, true);
for found in matches.into_iter().flatten() {
    // found.world is the matching branch; found.selected_trinket is its recipe.
}
```

This API is independent of WASM, JSON, and frontend settings. The WASM adapter
only transports the web setting and serializes the engine's results. Native
frontends can adopt the same API without implementing their own branching.

## Web search lifecycle

Search results carry `selectedTrinket` and show the required +3 trinket next
to the seed. Scouting a result applies it automatically; clicking an offered
trinket still overrides the selection, including deselecting it.

The web coordinator snapshots `auto_apply_trinkets` alongside each search
query. A change in effective mode invalidates coverage reuse and causes a
fresh scan, retaining the existing Target Set. A refine in the same mode
rechecks every seed through the engine, including all eligible branches.

Results files preserve the setting as an optional top-level
`auto_apply_trinkets: true` field, outside the portable query. Import restores
the preference and permits the engine to rediscover the winning recipe.
Older/native importers ignore the extension; they cannot reproduce assisted
matches automatically until they adopt this feature. Share links continue to
carry requirements, while auto-apply remains a local performance preference.

Auto-apply expands matches per seed but costs more generation time. It does
not guarantee better time to a match. The web app hides the baseline-only
probability/time estimates while it is active. See the
[direct Rust benchmark](auto-trinkets-benchmark.md) for measured tradeoffs and
reproduction commands.
