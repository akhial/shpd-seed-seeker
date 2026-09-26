# Search query format

Search queries are JSON documents. Pass a query file to the CLI with
`seed-seeker --items requirements.json` (or `-i requirements.json`). See the
[CLI getting started guide](../README.md#cli) for build and run commands.

The reference below uses `?` for optional fields, `|` for alternatives,
`..` for inclusive ranges, `=` for defaults, and `...` for repeated entries.

```jsonc
// ? optional · | alternatives · .. inclusive range · = default · ... repeat
{
  "max_depth"?: 1..24 = 24,
  "require_blacksmith"?: true | false = false,
  "exclude_blacksmith_rewards"?: true | false = false,
  // Minimum resin from extra uncursed wands, within max_depth. Wands used
  // by other requirements cannot also provide resin. Each yields
  // 2 * (upgrade + 1): +0 gives 2, +1 gives 4, +2 gives 6, etc.
  // A resin-only query may use an empty requirements array.
  // "auto" upgrades kept wands to +3, excluding reforge copies and exclude_resin wands:
  // +0 needs 6, +1 needs 5, +2 needs 3, and +3 or higher needs none.
  "arcane_resin"?: 0..65535 | "auto" = 0,
  "arcane_resin_filter"?: {
    "uncursed"?: true | false = true,
    "max_depth"?: 1..24,
    "source"?: "heap" | "chest" | ...,
    // Assumes the Mage recovers and dismantles the starting Magic Missile
    // wand with Wand Preservation: one +0 wand, worth 2 resin, regardless
    // of staff level. This explicit credit is independent of donor filters.
    "include_mage_wand"?: true | false = false
  },
  // The run's Wandmaker (floors 7-9) must ask for this quest item.
  "wandmaker_quest"?: "corpse_dust" | "elemental_embers" | "rotberry",

  "challenges"?: [
    (
      "on_diet" | "faith_is_my_armor" | "pharmacophobia" | "barren_land" |
      "swarm_intelligence" | "into_darkness" | "forbidden_runes" |
      "hostile_champions" | "badder_bosses"
    ),
    ...
  ] = [],

  "requirements": [
    // Each entry is a requirement, or an "any_of" group that is satisfied
    // when any single member matches:
    //   { "any_of": [ { "item": "spear", "upgrade": 3 },
    //                 { "item": "shuriken", "upgrade": 2 },
    //                 { "item": "sword", "upgrade": 1 } ] }
    // Members use the requirement schema below, except "level_sum".
    {
      // Supply "item", "kind", or both; when both are present they must agree.
      // "weapon" matches melee and thrown weapons alike; "melee_weapon" and
      // "thrown_weapon" narrow it to one class.
      // Ordinary wand only: reserve it, but omit its Auto upgrade cost.
      // Useful for imbuing; resin upgrades never transfer to the staff.
      "exclude_resin"?: true | false = false,
      "kind"?: "weapon" | "melee_weapon" | "thrown_weapon" | "armor" | "wand" | "ring" | "trinket" | "artifact",
      "item"?:
        // Weapons
        "worn_shortsword" | "cudgel" | "gloves" | "rapier" | "dagger" |
        "shortsword" | "hand_axe" | "spear" | "quarterstaff" | "dirk" | "sickle" |
        "sword" | "mace" | "scimitar" | "round_shield" | "sai" | "whip" |
        "longsword" | "battle_axe" | "flail" | "runic_blade" | "assassins_blade" |
        "crossbow" | "katana" | "greatsword" | "war_hammer" | "glaive" | "greataxe" |
        "greatshield" | "gauntlet" | "war_scythe" | "throwing_stone" |
        "throwing_knife" | "throwing_spike" | "fishing_spear" | "throwing_club" |
        "shuriken" | "throwing_spear" | "kunai" | "bolas" | "javelin" | "tomahawk" |
        "heavy_boomerang" | "trident" | "throwing_hammer" | "force_cube" | "rot_dart" |
        "incendiary_dart" | "adrenaline_dart" | "healing_dart" | "chilling_dart" |
        "shocking_dart" | "poison_dart" | "cleansing_dart" | "paralytic_dart" |
        "holy_dart" | "displacing_dart" | "blinding_dart" |
        // Armor
        "cloth_armor" | "leather_armor" | "mail_armor" | "scale_armor" | "plate_armor" |
        // Wands
        "wand_magic_missile" | "wand_fireblast" | "wand_frost" | "wand_lightning" |
        "wand_disintegration" | "wand_prismatic_light" | "wand_corrosion" |
        "wand_living_earth" | "wand_blast_wave" | "wand_corruption" | "wand_warding" |
        "wand_regrowth" | "wand_transfusion" |
        // Rings
        "ring_accuracy" | "ring_arcana" | "ring_elements" | "ring_energy" |
        "ring_evasion" | "ring_force" | "ring_furor" | "ring_haste" | "ring_might" |
        "ring_sharpshooting" | "ring_tenacity" | "ring_wealth",

      // Named artifact: accept a natural find or the first N remaining deck draws
      // after generating this requirement's floor limit. Requires a donor artifact.
      "artifact_transmutations"?: 0..10 = 0,
      // Named trinket: accept an initial offer or the first N later deck draws.
      "trinket_transmutations"?: 0..13 = 0,

      // Tier filters apply only to wildcard weapon/armor requirements.
      "tier"?:
        "any" |
        { "exact": 2..5 } |
        { "at_least": 3..4 } |
        { "at_most": 3..4 }
        = "any",

      // Everything reaches +4; a tier-4 weapon, melee or thrown, reaches +5.
      // "any" and effect names are case-insensitive.
      "upgrade"?:
        "any" | 1..5 |
        { "exact": 1..5 } |
        { "at_least": 0..5 }
        = "any",

      // The effect must belong to the selected weapon or armor kind. A list
      // matches any one of its entries, and "any_enchantment" is shorthand
      // for every non-curse enchantment or glyph.
      "effect"?: <name> | [<name>, ...] | "any_enchantment", where <name> is
        // Weapon enchantments
        "Blazing" | "Chilling" | "Kinetic" | "Shocking" | "Blocking" | "Blooming" |
        "Elastic" | "Lucky" | "Projecting" | "Unstable" | "Corrupting" | "Grim" |
        "Vampiric" | "Venomous" | "Eldritch" | "Vorpal" | "Crystal" |
        // Weapon curses
        "Annoying" | "Displacing" | "Dazzling" | "Explosive" | "Sacrificial" |
        "Wayward" | "Polarized" | "Friendly" | "Pressurized" | "Wondrous" |
        // Armor glyphs
        "Obfuscation" | "Swiftness" | "Viscosity" | "Potential" | "Brimstone" | "Stone" |
        "Entanglement" | "Repulsion" | "Camouflage" | "Flow" | "Affection" |
        "Anti-Magic" | "Thorns" |
        // Armor curses
        "Anti-Entropy" | "Corrosion" | "Displacement" | "Metabolism" | "Multiplicity" |
        "Stench" | "Overgrowth" | "Bulk",

      // true cannot be combined with a curses-only effect list.
      "uncursed"?: true | false = false,
      "source"?:
        "heap" | "chest" | "locked_chest" | "crystal_chest" | "tomb" | "skeleton" |
        "sacrificial_fire" | "mimic" | "golden_mimic" | "crystal_mimic" | "statue" |
        "armored_statue" | "shop" | "ghost_reward" | "wandmaker_reward" |
        "blacksmith_reward" |
        // The Imp's six vault prizes, and the equipment in the vault's
        // treasure rooms; the player carries exactly one item out of either.
        "imp_reward" | "vault_treasure",
      // Equal groups must resolve to the same kind and item ID.
      "identity_group"?: 1..255,
      "max_depth"?: 1..24 = query.max_depth,
      // Requirements sharing a group are matched by distinct items whose
      // *levels* — each item's upgrade plus one — add up to at least
      // "at_least", on top of each member's own upgrade filter. Members are
      // optional: any subset that reaches the total satisfies the group, so
      // "up to two Rings of Might reaching 5 levels" (a +1 and a +2, or a
      // single +4) is:
      //   { "item": "ring_might", "level_sum": { "group": 1, "at_least": 5 } },
      //   { "item": "ring_might", "level_sum": { "group": 1, "at_least": 5 } }
      // All members of one group must agree on "at_least". A same-item group
      // ("identity_group") is a stack: one member — or the members of one
      // "any_of" group — may name the item and its qualities; every other
      // member must be a plain entry of the same kind.
      "level_sum"?: { "group": 1..255, "at_least": 1..255 }
    },
    ...
  ]
}
```

## Ring of Wealth farming floors

On Android, open **Search settings** from the Finder and use the **7 / 17 / 22**
segmented control under **Rooms and feelings → Ring of Wealth farming floors**.
On web, Linux, macOS, and Windows, expand **Rooms and feelings** and select
**Floor 7**, **Floor 17**, or **Floor 22**. Each selected floor must be dark and contain a garden or secret garden.
The floors are independent: selecting several requires all of them. Selecting a
floor raises the search limit if needed; clearing it leaves that limit in place.
These filters work on their own or alongside item requirements. They do not
implicitly require a Ring of Wealth. Scout labels qualifying floors **Garden**,
using each platform’s floor-header styling alongside its quest details.

Drafts, presets, share links, result exports, and search refinement preserve the
filters. Imported room and feeling filters also remain visible and removable.
The CLI accepts the same query document, for example:

```json
{
  "max_depth": 17,
  "requirements": [],
  "floor_requirements": [
    { "depth": 7, "feeling": "dark", "any_rooms": ["garden", "secret_garden"] },
    { "depth": 17, "feeling": "dark", "any_rooms": ["garden", "secret_garden"] }
  ]
}
```

## Blanket requirements

In any app, add the items you need under **Requirements**, then add an
extra filter under **Blanket Requirements**. Each blanket must match at least
one of the items chosen to fulfill the ordinary requirements or supply Arcane
Resin. Donor witnesses obey the resin donor filters as well as the blanket's
filters; Auto does not add their upgrade cost. A blanket does not ask for an
additional item. Separate blankets may be satisfied by the same item
or different chosen items; filters within one blanket apply to the same item.

For Lightning, Disintegration, and Frost at +2 or higher, with one of those
three supplied by the Wandmaker at +3:

```json
{
  "requirements": [
    { "item": "wand_lightning", "upgrade": { "at_least": 2 } },
    { "item": "wand_disintegration", "upgrade": { "at_least": 2 } },
    { "item": "wand_frost", "upgrade": { "at_least": 2 } },
    { "kind": "wand", "upgrade": 3, "source": "wandmaker_reward", "blanket": true }
  ]
}
```

An unrelated +3 wand does not satisfy this blanket unless it supplies required
Arcane Resin. Either/or alternatives are supported within the blanket section.
Blankets use the usual source,
upgrade, effect, uncursed, tier, and floor filters, but do not request stacks,
combined levels, or trinket selection. At least one ordinary requirement is
required. Saved queries, results files, and share links preserve blankets;
blanket share links use format 9 and require an app that supports it.

The probability estimate intersects blanket filters with ordinary item and
resin donor filters and considers their possible witnesses. Donor witnesses
contribute resin once, can satisfy several blankets, and add no Auto upgrade
cost. Auto donor witnesses require a kept, non-excluded wand below +3. Overlap between
those ways is approximated conditional on the ordinary query, so a blanket
never raises the estimate above the ordinary query's rate. Existing
approximations for alternatives, combined levels, and item supply still apply. Queries with more
than 128 intermediate witness combinations show an unavailable estimate.
The estimate is also unavailable when the combined-level approximation drops
an optional member needed to witness a blanket.

See [artifact search](artifact-search.md) for artifact IDs, deck semantics, and limitations.
