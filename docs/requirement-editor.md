# Requirement editor

Every Seed Seeker app edits its query's requirements the same way: a
**board** of chips, either/or clusters and stacks in two sections (ordinary
and blanket requirements), and a **sheet** that opens when a chip is tapped
or added. The rules behind both — how a flat requirement list folds into
chips, what a drag, a badge tap or a menu writes back, how a stack's copies
are encoded, what every chip, badge and control says, and what is wrong
with a list — live once, in the shared core
(`crates/seedfinder-core/src/editor/`). Each app used to carry its own port
of them, and the ports drifted.

The core owns the rules and the words. Every app keeps what is its own:
rendering, gestures and hit-testing, animation, sprites and colours, dialog
chrome (sheet titles and button labels), persistence, the canonical query
codec, and query-level settings and checks (search depth, floor filters,
challenges, "the query is empty").

Linux calls the typed Rust API directly. Every other app goes through two
stateless JSON envelopes that project the same API without adding rules:

| App | Entry points |
| --- | --- |
| Web | `requirement_board`, `requirement_editor`, `editor_limits` from the lean `seedfinder_editor` WebAssembly module (`web/src/engine/editor-pkg`); `requirement_board` and `requirement_editor` also from the engine module (`web/src/engine/pkg`) for workers and tests |
| macOS, iOS, Windows | `seedfinder_requirement_board`, `seedfinder_requirement_editor` (C ABI, `crates/seedfinder-ffi/include/seedfinder.h`) |
| Android | `JniBindings.requirementBoard`, `JniBindings.requirementEditor` (JNI, UTF-8 JSON bytes in and out) |
| Linux (GTK) | `shpd_seedfinder_core::editor` directly |

## Rows and keys

The editor works on **one flat list of rows** holding both board sections; a
row's `blanket` flag says which section it sits in. Every edit sees the whole
list, and alternative labels are allocated across the whole list. New rows
are appended to the end; the editor never reorders sections.

A row is the canonical requirement object of the
[search query format](search-query-format.md) — exactly as the core's
query encoder writes one entry of `requirements` — plus two fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `key` | integer | The app's identity for the row, stable across edits. |
| `alternative_group` | integer ≥ 1, optional | The either/or cluster the row belongs to. The query document writes clusters as `any_of` entries; a row carries its label instead. |

```json
{"key": 3, "kind": "wand", "item": "wand_fireblast", "upgrade": 3, "alternative_group": 1}
```

Rows are read with the query document's own reader, so a row and a document
entry never disagree on a field, and are written back in its canonical
spelling (defaults omitted, effects in catalog order). A row no edit
touched comes back as the same requirement, but not necessarily as the same
bytes — the fields, `key` among them, may come in another order, and so may
the effects an app listed — so compare rows as JSON values, or rely on
`changed`.

### Keys

- Keys are integers from 1 to 2⁵³ − 1 (`MAX_KEY`), the largest integer every
  platform, JavaScript included, holds exactly. Apps re-key their rows
  1…n when they load or import a list.
- The core still repairs a list defensively. A key of zero, a key out of
  range, or a later duplicate of a key is re-keyed deterministically, in list
  order; the first occurrence of a key always keeps it. Every repair is
  reported in `rekeyed` as `[old, new]` and sets `changed`. The request's own
  references — edit keys, the sheet's key — follow the same repair: a zero
  or out-of-range key names its row's new key.
- New keys: `max(largest key, next_key − 1) + 1`, or the smallest unused
  key once that would pass `MAX_KEY`. Every answer returns `next_key`, one
  past the largest key or the request's `next_key` when that is larger, so
  an app that claims keys up front keeps its counter moving forward.
- A save naming a key that is not in the list adds the new row **with that
  key**. Saving a stack reuses the keys of the copies it took down, so
  saving an unchanged chip gives back identical rows.

### Labels

- Alternative labels may be any integer ≥ 1 on input (Android numbers
  clusters without a bound). If any label passes 255, every cluster is
  relabelled 1, 2, … in first-appearance order and `changed` is set.
- Stack (`identity_group`) and combined-level (`level_sum.group`) labels
  must be 1–255 on input; the editor never mints one above 4.
- Whatever an edit or a save writes from rows that each pass the engine's
  requirement validation is itself valid: every row passes validation,
  keeps its stack and combined-level labels in 1–4, carries no such label
  on a trinket, artifact or blanket row, never combines a level sum with an
  alternative label, and reads back through the row codec unchanged. Several
  platform models refuse to construct a requirement breaking these rules, so
  the guarantee is property-tested.

### Unreadable rows

A row whose requirement cannot be read (an item a later catalog renamed,
say — only the web's lenient store keeps such rows) does not fail the
board. It is carried through **verbatim** at its place in the list, shown as
a standalone entry `r<key>` whose single chip is named `Unknown requirement`
and has `kind` and `family` `null`, and its reason is the entry's, the
chip's and the problem list's problem. It takes part in no relationship: no
chip can join it, and it joins nothing. Only `remove` and `remove_item`
apply to it; any other edit naming it does nothing, and the sheet refuses to
open it or save onto it. Its key and group labels stay its own: keys the
editor mints never land on an unreadable row's key, and no new cluster,
stack or combined level takes a label it holds (when wide labels are
compacted, its alternative label is relabelled with the others). It is a
row of its section all the same: `counts` counts it, and so does the
list-level problem.

A row that is not an object, or has no integer `key`, fails the request —
nothing could name it.

## The board: `requirement_board`

### Request

```json
{"rows": [ROW], "next_key": 7, "edits": [EDIT], "resin": RESIN}
```

`rows` is required; `next_key`, `edits` (default none) and `resin` (the
query's Arcane Resin condition, default `null`) are optional, and `null`
means the same as leaving one out. Unknown fields are errors, in an edit
too.

**RESIN** is the query's Arcane Resin condition:

```json
{"amount": 4, "filter": {"uncursed": true, "max_depth": 14, "source": null, "include_mage_wand": false}}
```

`amount` is 1–65535 or `"auto"`. `filter` may be left out or `null`, and so
may each of its fields; they default to uncursed donors, no floor limit, any
source and no Magic Missile credit. `source` is a source name of the query
format (`"locked_chest"`).

**EDIT**, applied in order:

| Edit | Effect |
| --- | --- |
| `{"type": "normalize"}` | Rewrites the list into its canonical encoding. Send once when a list is loaded or imported. |
| `{"type": "join", "source": K, "target": K}` | Makes `source` an either/or alternative of `target` (any member of a chip or cluster). The source moves after the cluster's last member. |
| `{"type": "detach", "key": K}` | Takes a cluster member out on its own. It leaves the cluster's stack behind. |
| `{"type": "remove", "key": K}` | Removes a cluster member, or a chip's whole entry. |
| `{"type": "remove_item", "key": K}` | Removes the whole entry holding `K`: members and hidden copies. |
| `{"type": "set_count", "key": K, "count": n}` | How many items the entry asks for, clamped to 1–3. |
| `{"type": "set_total", "key": K, "total": n \| null}` | Sets or clears the stack's combined level, clamped to 1–`level_capacity`. |
| `{"type": "toggle_levels", "key": K}` | Turns counting levels on (at `default_total`) or off. |
| `{"type": "set_copy_depth", "key": K, "max_depth": n \| null}` | Sets or clears the floor limit of the stack's hidden copies; an empty boss floor snaps to the floor below. |
| `{"type": "save", "key": K \| null, "requirement": ROW_WITHOUT_KEY, "count": n, "total": n \| null, "copy_depth": n \| null}` | Stores a requirement with its stack's shape. `null`, or a key not in the list, appends a new row. The sheet's `save` sends this for you. |

Keys name **visible** rows — a chip, or one member of a cluster — never a
stack's hidden copies; an edit naming an unknown key changes nothing. Every
row of any list, a hand-written one included, is a member or a hidden copy
of exactly one board entry, so every row can be seen and removed.

A `save` of a cluster member follows the join rules: moving it into a
category the rest of its cluster does not share is refused with
`mixed_category_stack` when the cluster is a stack, and clears the
cluster's leftover stack labels when it is not.

### No-op and refused edits

- An edit that changes nothing returns the rows **verbatim**, `changed:
  false`, and does not normalize: the same row, the row's own cluster, a
  row of the other section, an unknown key, `detach` on a chip that is not
  in a cluster, a count, total or copy floor that is already so. A request
  without edits returns the rows verbatim after key repair. `changed`
  compares the final rows with the request's, so edits that undo each other
  within one request change nothing. Apps write the rows back only when
  `changed` is true.
- A refused edit stops the sequence (earlier edits stay applied) and says
  why in `refused`:

  | `reason` | `message` |
  | --- | --- |
  | `mixed_category_stack` | `Copies can only be grouped with the same item type.` |
  | `blanket_total` | `A blanket requirement cannot count levels together.` |
  | `no_free_group` | `Every group label is in use. Remove a stack or a combined level first.` |

### Response

```json
{
  "rows": [ROW], "next_key": 9, "changed": true, "rekeyed": [[0, 5]],
  "focus": 5, "refused": null,
  "items": [ITEM], "counts": {"ordinary": 3, "blanket": 1},
  "problems": [PROBLEM], "resin": RESIN_CHIP
}
```

| Field | Meaning |
| --- | --- |
| `rows` | The list after the edits. |
| `changed` | Whether the rows differ from the request's (a key repair or a label compaction counts). |
| `focus` | The row to follow — scroll to, highlight, announce: the joined source, the detached row, the anchor of the entry an edit reshaped or a save landed in, as the last edit that applied left it. A save names its entry even when it stored what was already there (`changed: false`), so a closing sheet can return to its chip. `null` after a removal and when no edit applied. Always a visible row. |
| `refused` | `{"reason", "message"}` of a refused edit, else `null`. |
| `items` | The board's entries in list order, both sections together; split them by `blanket`. |
| `counts` | How many entries each section shows (clusters and stacks count once). |
| `problems` | Everything wrong with the list, see [Problems](#problems). |
| `resin` | The Arcane Resin chip when the request carried `resin`, else `null`. |

**ITEM**, one board entry:

| Field | Meaning |
| --- | --- |
| `id` | `"r17"` for a chip (its anchor row's key), `"c3"` for a cluster (its label). Stable while the entry survives an edit. |
| `blanket` | The entry's section. |
| `cluster`, `label` | A cluster's alternative label and caption (`Any of 3`); `null` for a chip. |
| `members` | The visible rows' keys: one for a chip, every member of a cluster. |
| `extras` | The hidden copies' keys behind the stack badge. |
| `stack` | What the count and combined-level steppers offer: `count`, `max` (3), `can_grow`, `can_change_count`, `total`, `can_count_levels`, `level_capacity`, `default_total`, `copy_depth`, `can_set_copy_depth`, `count_text` (`×2`, or `≤2` while counting levels — present even at ×1 for steppers), `total_text` (`Σ ≥ 5`, `Σ ≥ 0` without a total). |
| `badges` | The badges shown at rest: `count` when the entry asks for more than one item, `total` when it counts levels; each `{"text", "compact_text", "tooltip"}` (`×3` / `3 of the same kind`; `Σ ≥ 5`, compact `Σ≥5`, `Levels add to at least 5 (a +0 item counts 1)`), else `null`. |
| `chips` | One CHIP per member. |
| `problem` | The first problem touching any member or hidden copy, so a problem on a folded-away copy still shows. |

**CHIP**, one visible row:

| Field | Meaning |
| --- | --- |
| `key` | The row's key (0 in a sheet preview). |
| `name` | The short name beside the sprite: the item (`Ring of Might`), or `Any weapon`, `Any melee`, `Any thrown`, `Any armor`, `Any wand`, `Any ring`, `Trinket`, `Artifact`. |
| `title` | The full title the popover and sheet lead with: the item, or `Any Tier 3+ melee weapon`. |
| `item` | The item's stable id, or `null` for a wildcard. |
| `kind`, `family` | The kind (`melee_weapon`) and family (`weapon`) for the sprite. |
| `tags` | Qualifiers after the name, in order: `Transmute ≤3` or `choose at +3`, the tier (wildcards only: `T3`, `T3+`, `T≤3`), the upgrade (`+3`, `+3↑`, style `upgrade`), `F≤9`. Each `{"text", "style": "plain" \| "upgrade"}`. |
| `trailing_tags` | Qualifiers after the effect cue: `No resin`. |
| `effect` | `{"label", "effects", "any_enchantment", "curses_only"}` — `any enchantment` (`any glyph` on armor), one effect's name, or `effect: A/B/C`; `effects` in catalog order, the full set for any enchantment. `null` for any effect. |
| `uncursed` | Cursed items are ruled out (drawn as a check mark). |
| `details` | The popover's detail line as parts: `within 3 transmutations`, `choose at +3`, the upgrade (`exactly +3`, `+3 or higher`, `any upgrade` — left out while counting levels and on trinkets and artifacts), the effect, `uncursed`, `excluded from Auto resin`, the source (`Locked chest`), `floors 1–9`. |
| `relations` | The popover's relation lines, each `{"glyph": "or" \| "sum" \| "times", "text"}`: the cluster's other members, `up to 2 — levels add to ≥ 5`, `3 of the same kind — the extra copies: any upgrade, floors 1–4`. |
| `description` | The accessibility label: the title, then the details. |
| `problem` | The row's own first problem, else the first problem between rows blaming it; the anchor also speaks for its hidden copies. |
| `in_cluster`, `can_detach` | A cluster member, which "On its own" (`detach`) applies to. |
| `join` | The visible rows this chip may join, in list order — what "Either/or with…" menus, pick mode, accessibility actions and drag hover read. |
| `refuse` | The visible rows a join onto is refused, each `{"key", "reason", "message"}`, for hover feedback. |

Drops are decided from the chips: onto a row or cluster, `join` it when the
target is in `join`, show the message when it is in `refuse`, else do
nothing; onto the empty board of the chip's own section, `detach` a cluster
member (`can_detach`) and leave a lone chip where it is; onto the remove
target, `remove`.

**RESIN_CHIP**: `{"name": "Arcane Resin", "tags", "uncursed", "tooltip",
"amount_tooltip", "details", "description"}` — tags `Auto` or `≥N`, then
`Mage +2`, `F≤N`; `tooltip` is the chip's hover text, the donors' source
(`Locked chest`), or `null` for any source; `amount_tooltip` is the amount
tag's, which explains Auto (`Enough resin to upgrade kept wands to +3,
excluding No resin wands and reforge copies`), or `null` for a fixed
amount.

### Problems

`problems` lists every problem, each `{"message", "keys", "scope"}`:

1. each row's own problems (`scope: "row"`, `keys` = that row), in list
   order, hidden copies and unreadable rows included;
2. then problems between rows (`"group"`: a stack or combined level that
   disagrees, an either/or group mixing sections, a stack mixing kinds or
   constraining two copies), blaming every row of the group;
3. then the list's own (`"list"`, `keys` empty): `Add at least one ordinary
   requirement.` when blanket rows have no ordinary row to constrain.

The wording is the web editor's. Apps gate Start and Share on their own
query-level checks first, then on the first problem; the web prefixes it
with `Requirement N: `, N being the 1-based position of its first key.

## The sheet: `requirement_editor`

A sheet is held as a **draft**: an opaque JSON string (it carries `"v": 1`)
that the app stores and sends back untouched with the next request. Three
requests:

```json
{"op": "open", "rows": [ROW], "key": K, "blanket": false, "resin": RESIN, "offer_resin": true, "open_resin": false}
{"op": "change", "draft": DRAFT, "change": CHANGE}
{"op": "save", "draft": DRAFT, "rows": [ROW], "next_key": 7}
```

- `open` — on the row `key` (a hidden copy's key opens its entry's anchor),
  or on a new chip with `key: null` (or a key not in the list, which the new
  row will take); `blanket` picks the new chip's section; `resin` is the
  query's current resin condition, which seeds the resin section;
  `offer_resin` offers Arcane Resin among the wands; `open_resin` opens the
  query's resin chip. Everything but `rows` defaults to `null`/`false`, and
  `null` means the default.
- `change` — applies one control the user moved.
- `save` — saves the draft onto `rows`, the list as it is now. When the row
  the sheet was opened on has since become a hidden copy of another chip,
  the save adds a new chip rather than vanishing into the copy.

Open and change answer `{"draft": DRAFT, "form": FORM}`. A save answers

```json
{"saved": {"rows": [ROW], "next_key": 9, "changed": true, "rekeyed": [], "focus": 8,
           "resin": {"set": RESIN} | {"clear": true} | null}}
```

or, when the draft cannot be saved, `{"draft", "form"}` again with the
reasons in `form.errors`. `resin` is `{"set": RESIN}` when Arcane Resin was
the picked item (the wand chip the sheet was opened on is removed from the
rows), `{"clear": true}` when the resin chip was saved as a requirement, and
`null` otherwise.

**CHANGE** is `{"type": ..., "value": ...}`, `value` one type per change:

| `type` | `value` |
| --- | --- |
| `set_category` | a family: `weapon`, `armor`, `wand`, `ring`, `trinket`, `artifact` |
| `set_weapon_type` | `any`, `melee`, `thrown` |
| `set_kind` | a kind: a family, `melee_weapon` or `thrown_weapon` |
| `set_item` | an item's stable id, `null` for the wildcard, or `arcane_resin` |
| `set_tier_mode` | `any`, `exact`, `at_least`, `at_most` |
| `set_upgrade_mode` | `any`, `exact`, `at_least` |
| `set_effect_mode` | `any`, `any_enchantment`, `specific` |
| `toggle_effect` | an effect name (`Blazing`) |
| `set_source` | a source name, or `null` |
| `set_tier`, `set_upgrade`, `set_floor_limit`, `set_transmutations`, `set_count`, `set_copy_depth`, `set_total` | an integer 0–255 |
| `set_uncursed`, `set_floor_limit_enabled`, `set_exclude_resin`, `set_transmutations_enabled`, `set_select_trinket`, `set_copy_depth_enabled`, `set_count_levels`, `set_resin_auto`, `set_include_mage_wand` | `true` or `false` |
| `set_resin_amount` | the typed number, or `null` for an empty field |

A change to a control the form hides — a tier on a named item, a stack in a
cluster — changes nothing. Values are clamped into range; floor sliders
step over the empty boss floors (a single step up onto 5, 10 or 15
continues to 6, 11 or 16; every other move snaps down).

**FORM** is everything the sheet shows. Every numeric control carries
`min ≤ value ≤ max`, even while hidden, and every picker option is
`{"value", "label", "group", "hidden"}` — `hidden` marks a choice offered
only because the draft already names it (a tier-1 item from an imported
query).

| Field | Meaning |
| --- | --- |
| `v`, `mode` (`new` \| `edit`), `origin` (`{"type": "new"}`, `{"type": "row", "key": K}`, `{"type": "resin"}`), `blanket`, `in_cluster`, `resin_picked` | What the dialog chrome — title and button labels, which apps own — derives from. |
| `title` | The sheet header's title: the requirement's (`Any Tier 3+ melee weapon`, `Rat Skull`), or `Arcane Resin` while the resin is picked. Unlike `preview` it is there while the draft has errors; the sprite follows `item` and `kind`. |
| `preview` | The CHIP a save would produce (key 0, no join candidates), or `null` while there are errors or the resin is picked. |
| `category`, `kind`, `weapon_type`, `item`, `source` | Pickers: `{"visible", "value", "options"}`. `item` lists the wildcard (`Any melee weapon`) unless the family always names one, `Arcane Resin` when offered, then the items — weapons grouped `Tier 2`…`Tier 5`. |
| `tier`, `upgrade` | `{"visible", "mode", "modes", "value", "min", "max", "value_label"}` (`Tier 3 or higher`, `+2 or higher`). |
| `effect` | `{"visible", "mode", "modes", "choices", "groups", "caption"}`; each choice `{"value", "label", "group": "enchantment" \| "curse", "selected"}`, curses listed only while the item may be cursed. |
| `uncursed`, `exclude_resin`, `select_trinket` | Check boxes: `{"visible", "value", "label"}`. |
| `floor_limit` | `{"visible", "enabled", "value", "options", "label", "value_label"}`; options skip the empty boss floors. |
| `transmutations` | `{"visible", "enabled", "value", "min", "max", "label", "caption", "value_label"}`. |
| `stack` | `{"visible", "count", "min", "max", "value_label", "copy_depth", "count_levels"}`, with `copy_depth` a floor toggle and `count_levels` a range toggle (`≥ 5 across up to 2`). |
| `resin` | `{"visible", "auto", "amount", "include_mage_wand"}`, `amount` the number as typed. |
| `errors`, `can_save` | Why the draft cannot be saved, in the order to show them. |

A draft's errors are its requirement's own problems, a trinket another
ordinary row already names (`This trinket is already required. Each
trinket appears only once in the deck.`), a resin amount that is not a
whole number from 1 to 65535 (`Enter an amount from 1 to 65535.`), and the
**save guard**: the save is tried on the rows, and any problem that blames
the saved row and did not before becomes an error. Problems the list
already had, or that blame other rows only, never block a save.

## Errors

The envelopes never throw and never panic on a bad request. A request they
cannot read — not JSON, a missing or unknown field, an unknown edit or
change type, a value of the wrong type or out of range, an edit's
requirement that cannot be read, a draft of another version — is answered

```json
{"error": "edit 1: unknown item 'wand_of_wonders'"}
```

plus `"key": K` when the row the sheet was asked to open, or to save onto,
cannot be read. A draft that cannot be read means the app reopens the sheet
from its rows. The bindings keep that shape: the C ABI returns `0` with the
error document (and `-1` only for a null pointer or bytes that are not
UTF-8, `-2` if the editor panics); JNI answers the error document for
unreadable bytes too and throws `IllegalStateException` only if the editor
panics; WebAssembly returns the string (a panic there traps the module,
which is why every request is answered rather than refused).

## Engine limits

`engine_info` (C `seedfinder_engine_info`, JNI `engineInfo`, wasm
`engine_info` and the editor module's `editor_limits`) publishes the
editor's bounds in `limits`: `stackMax` (3), `trinketTransmutationsMax`
(13), `artifactTransmutationsMax` (10), beside the tier, upgrade and label
bounds. Apps read them there rather than hardcoding mirrors.

## Typed API (Linux)

`shpd_seedfinder_core::editor` exports the typed surface the envelopes
project:

| Item | Purpose |
| --- | --- |
| `Row { key, requirement }`, `MAX_KEY`, `STACK_MAX` | Rows and their bounds. |
| `apply(rows, next_key, edits) -> EditResult`, `Edit`, `EditResult`, `Refusal` | Board edits with key repair, no-op and refusal semantics as above. |
| `board_view(rows, resin) -> BoardView` and its views (`ItemView`, `ChipView`, `StackView`, `Badges`, `Tag`, `EffectBadge`, `Relation`, `Counts`, `ResinChip`) | Everything a board draws. |
| `board_items`, `BoardItem`, `ItemKey`, `join_candidates`, `drop_action`, `DropTarget`, `DropAction` | The fold itself and the drop policy. |
| `problems(rows)`, `row_problems(requirement)`, `Problem`, `ProblemScope` | The problem list. |
| `open`, `change`, `form`, `save`, `Draft`, `Change`, `Form`, `SaveResult`, `ResinOutcome`, `ResinState`, `ResinAmount` | The sheet. |
| `can_grow`, `can_change_count`, `can_count_levels`, `level_capacity`, `default_total`, `copy_depth`, `can_set_copy_depth`, `stack_view` | Stack rules. |
| `skip_boss_floor`, `compact_alternative_labels`, `labels` | Floor-slider stepping, label compaction, and every English phrase. |
| `requirement_board`, `requirement_editor` (feature `json-query`) | The envelopes. |

## Settled behaviour

The apps disagreed on these before the rules moved to the core; each is now
decided once.

| Topic | Behaviour |
| --- | --- |
| Joining across categories with a stack | Refused: `Copies can only be grouped with the same item type.` |
| Plain copies traded on a join | Only the joined chip's own copies become identity copies. |
| Joining across categories without a stack | Leftover identity labels are cleared; nothing is deleted. |
| A stacked cluster member saved into another category | Refused like the join; without a stack, leftover labels are cleared. |
| Drop on the empty board | Detaches cluster members only; a lone chip stays. |
| Combined level on a blanket | Refused. |
| A cluster's stack label | Never spread onto trinket, artifact or blanket members. |
| Copy contents | Built from defaults; plain copies keep the melee/thrown narrowing; resin exclusion, blanket, trinket selection and transmutations are never copied. |
| Key lookup | Visible members only, never hidden copies. |
| Saving an unchanged chip | Identical rows, the same copy keys, `changed: false`. |
| New rows | Appended; sections are never reordered. |
| Wildcard chip names | `Any melee`, `Any thrown`. |
| Titles | `Any Tier 3 weapon`, `Any Tier 3+ weapon`, `Any Tier 3 or lower weapon`. |
| Item names | Title case, as in the catalog asset. |
| Source labels | Sentence case (`Locked chest`). |
| Tag order | Transmutations or `choose at +3`, tier, upgrade, floor, then the effect cue, then `No resin`. |
| Combined-level badge | `Σ ≥ T`, compact `Σ≥T`. |
| Armor's any-effect | `any glyph`. |
| Default combined level | The item count, within the capacity. |
| Level capacity | The stack's ring capacity, or its group's attainable capacity once counting. |
| New chip | Any weapon; a new blanket takes the first ordinary row's kind and melee/thrown narrowing. |
| Category switch | Keeps source, floor and uncursed where the new family has them; resets the rest. |
| Default tier | 3. |
| Upgrade bounds | Exactly +1…max, at least +1…max−1; a stored "+0 or higher" opens as any, "+max or higher" as exactly max. |
| "Specific…" with nothing ticked | Allowed; saves as any effect. |
| Floor pickers | Skip the empty boss floors. |
| Artifact transmutations | Supported, 1–10. |
| Save guard | A save that newly breaks the list around the saved row is refused. |
| Resin section | Seeded from the query's resin (uncursed donors by default), kept apart from the wand draft. |
| Dialog chrome | App-owned. |
| Chip problems | The row's own, then cross-row blame; hidden copies surface on their entry. |

### Known limitations

The apps shared these behaviours before the move, and the core keeps them
for now:

- A cluster whose stack shrinks to ×1 keeps its stack label, which still
  uses up one of the four labels.
- Joining a stacked chip onto a stacked cluster of the same category takes
  the chip's stack label for the whole cluster; the cluster's old copies
  lose theirs and become a standalone wildcard chip.
- Setting a combined level gives every copy the anchor's floor limit, and
  clearing it removes the copies' limits.
- Joining a stack that counts levels leaves its other members behind as a
  chip of their own.
- Joining a stack-labelled cluster member onto a chip of the same category
  can leave two constrained members in one stack, which the problem list
  reports.

## Golden fixtures

`crates/seedfinder-core/tests/fixtures/editor/*.json` pins representative
request/response pairs for both envelopes: the board tour, the four stack
encodings, joins (traded, refused), detach and removals, copy floors,
combined levels, saves (new and unchanged), problems, key repair and label
compaction, unreadable rows, the sheet's open/change/save flow, the resin
flows, and the error envelopes. Each file is

```json
{"about": "...", "envelope": "requirement_board", "request": {...}, "response": {...}}
```

A request is sent as its JSON text — except a request stored as a string,
which is sent as that string (the fixtures for text that is not JSON at
all). Sheet requests carry the very draft string an earlier response
returned. App tests replay the files through their own binding and compare
the answers.

The core test `tests/editor_fixtures.rs` regenerates every pair and fails
when a file drifts. After a deliberate change, review the answers and
rewrite the files:

```sh
UPDATE_EDITOR_FIXTURES=1 cargo test -p shpd-seedfinder-core --test editor_fixtures
```
