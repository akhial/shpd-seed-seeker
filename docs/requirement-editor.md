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
and board chrome (sheet titles, card titles and button labels, menu items,
drag captions), persistence, the canonical query codec, and query-level
settings and checks (search depth, floor filters, challenges, "the query is
empty").

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

- Alternative labels may be any whole number on input (Android numbers
  clusters without a bound). If any label passes 255, every cluster is
  relabelled 1, 2, … in first-appearance order and `changed` is set.
- Stack (`identity_group`) and combined-level (`level_sum.group`) labels
  are read as the query document reads them, 0–255; the editor never mints
  one above 4.
- `normalize`, and every edit that changes the rows, moves a label out of
  range onto a free one in range: a stack or combined-level label outside
  1–4 (the portable formats and every platform's model stop at 4, the
  engine rejects 0), or the reserved either/or label 0. Labels in range
  stay; each group out of range takes the lowest label no row — an
  unreadable row included — uses, in first-appearance order, so distinct
  groups stay distinct. A list with more stacks (or combined levels) than
  labels leaves the groups no label is left for as they were, and the
  problem list reports them (`A stack group must be 1 through 4.`, `A
  combined-level group must be 1 through 4.`): groups are never merged to
  fit.
- Whatever an edit or a save writes from rows that each pass the engine's
  requirement validation is itself valid: every row passes validation,
  keeps its stack and combined-level labels in 1–4 (unless the list holds
  more than four stacks or four combined levels), carries no such label on
  a trinket, artifact or blanket row, never combines a level sum with an
  alternative label, and reads back through the row codec unchanged. It is
  also canonical: `normalize` changes nothing on it. Several platform
  models refuse to construct a requirement breaking these rules, so the
  guarantee is property-tested.

### Unreadable rows

A row whose requirement cannot be read (an item a later catalog renamed,
say — only the web's lenient store keeps such rows) does not fail the
board. It is carried through **verbatim** at its place in the list, shown as
a standalone entry `r<key>` whose single chip is named `Unknown requirement`
and has `kind` and `family` `null`, and its reason is the entry's, the
chip's and the problem list's problem. It takes part in no relationship: no
chip can join it, and it joins nothing. Only `remove`, `remove_one` and
`remove_item` apply to it; any other edit naming it does nothing, and the
sheet refuses to open it or save onto it. Its key and group labels stay its own: keys the
editor mints never land on an unreadable row's key, and no new cluster,
stack or combined level takes a label it holds (when wide labels are
compacted, its alternative label is relabelled with the others) — and the
chips' `join` and `refuse` lists count those labels as taken, so a join
that would need one is listed in `refuse` with `no_free_group`, as the edit
answers (`board-unreadable-row-labels`). It is a
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

`amount` is 1–65535 or `"auto"`; any other amount fails the whole request
with an error envelope, so apps keep the query's amount in range (every
query codec already refuses one outside it). `filter` may be left out or
`null`, and so may each of its fields; they default to uncursed donors, no
floor limit, any source and no Magic Missile credit. `source` is a source
name of the query format (`"locked_chest"`).

**EDIT**, applied in order:

| Edit | Effect |
| --- | --- |
| `{"type": "normalize"}` | Rewrites the list into its canonical encoding, its labels in range among it (see [Labels](#labels)). See [When to normalize](#when-to-normalize). |
| `{"type": "join", "source": K, "target": K}` | Makes `source` an either/or alternative of `target` (any member of a chip or cluster). The source moves after the cluster's last member. One item moves: the source's own row, while the rest of its stack stays where it was — a lone chip's copies as their own entry, one fewer; a member's stack in its cluster, one fewer. A stacked lone target keeps its stack as a member of the new cluster; a target cluster's members keep theirs, and the source joins as a ×1 member. See [Joins](#joins). |
| `{"type": "detach", "key": K}` | Takes one item of a cluster member out on its own: the member's row, with its constraints. The rest of its stack stays in the cluster in its place, one fewer; a ×1 member leaves the cluster, and a cluster of one dissolves into a chip. |
| `{"type": "remove", "key": K}` | Removes the chip with its whole stack: a cluster member with its own copies (a stack it shares with other members stays with them), or a lone chip's whole entry. The chip menu's "Remove". |
| `{"type": "remove_one", "key": K}` | Removes one item of the chip — what a drag onto the remove target sends: a member ×N becomes ×(N−1), a ×1 member leaves its cluster (removed), a lone stack ×N becomes ×(N−1) (a combined level capped at what the rest can reach, or dropped at one ring), and a lone chip without copies is removed. |
| `{"type": "remove_item", "key": K}` | Removes the whole entry holding `K`: every member and every hidden copy. |
| `{"type": "set_count", "key": K, "count": n}` | How many items the chip asks for, clamped to 1–3 — a lone chip's stack, or a member's own. New copies take the floor limit the existing copies carry, not the chip's. |
| `{"type": "set_total", "key": K, "total": n \| null}` | Sets or clears the lone chip's combined level, clamped to 1–`level_capacity`. A cluster member counts no levels: nothing happens. |
| `{"type": "toggle_levels", "key": K}` | Turns counting levels on (at `default_total`) or off, on a lone chip. The chip and every copy keep their own floor limits both ways. |
| `{"type": "set_copy_depth", "key": K, "max_depth": n \| null}` | Sets or clears the floor limit of the chip's hidden copies — a lone chip's or a member's; an empty boss floor snaps to the floor below. |
| `{"type": "save", "key": K \| null, "requirement": ROW_WITHOUT_KEY, "count": n, "total": n \| null, "copy_depth": n \| null}` | Stores a requirement with its stack's shape — a cluster member's too, as its own stack. `null`, or a key not in the list, appends a new row. The sheet's `save` sends this for you. |

Keys name **visible** rows — a chip, or one member of a cluster — never a
stack's hidden copies; an edit naming an unknown key changes nothing. Every
row of any list, a hand-written one included, is a member or a hidden copy
of exactly one board entry, so every row can be seen and removed.

A `save` of a cluster member keeps it in its place in its cluster and gives
it the stack the sheet shows as its own, in its own kind — none when it
became a trinket, artifact or blanket, which never stack. Moving a member
into a category the rest of its cluster does not share is as good as any
other save: every copy names its own chip's kind.

### Stacks on chips

Every chip — a lone chip or a cluster member — has a stack of its own, and
every `×N` and `Σ` badge and every count stepper is a chip's: nothing is
drawn or counted per cluster. A member's stack is the engine's *member
stack* ([search query format](search-query-format.md#stacks)): its copies
are bare copies of its kind under a stack label the member carries, and
they count only when that member fills the cluster's slot. `{Frost ×2 |
Disintegration}` asks for two Wands of Frost, or one Wand of
Disintegration.

The canonical encoding of a cluster's stacks:

- members whose stacks ask for the same copies — the same count, kind and
  floor limits — share one label and one set of copies. A cluster whose
  members all share it is drawn as each member ×N (`{Frost ×2 |
  Disintegration ×2}`), which the engine reads as "two of whichever
  matched", the same thing;
- every other stacked member has a label of its own (`{Frost ×2 |
  Disintegration ×3}` takes two of the four);
- a ×1 member carries none.

An edit that changes one member's stack re-encodes the cluster as needed:
it gives the member a label of its own, shares another member's alike
stack, or drops its label. When a member needs a label and none of the
four is free, the edit is refused with `no_free_group` and the rows stay as
they were. A combined level stays on lone ring stacks: it cannot sit on a
cluster member.

### Joins

A join — a drag, pick mode, a menu's "Either/or with…", an accessibility
action — always sends the one `join` edit, and a join moves **one item**:
the source's own row, with its constraints. The picked-up chip (a drag
ghost, a lifted chip, a pick-mode or keyboard "moving" chip) shows that one
item — the chip's name and tags, without its `×N` or `Σ` badges.

- **The source gives up one item.** The rest of its stack stays where it
  was, one item fewer, with its own floor limits — as if the count had been
  stepped down by one. A lone chip's copies stay behind as an entry of
  their own: Wand of Disintegration ×2 onto Wand of Frost gives `{Frost |
  Disintegration}` and `Disintegration`; ×3 leaves `Disintegration ×2`;
  `Any wand +3 ×2` leaves `Any wand`. A member's stack stays in its cluster,
  in its place, as the same item without the constraints: Frost +2 of
  `{Frost +2 ×2 | Disintegration}` dragged onto Any wand +3 gives `{Frost |
  Disintegration}` and `{Any wand +3 | Frost +2}`. When the member shared
  its stack with an alike member, the rest is a new row, with a stack of
  its own if it still has copies.
- **A stacked lone target keeps its stack, as a member.** Disintegration
  onto Frost ×2 gives `{Frost ×2 | Disintegration}` — two Frosts, or one
  Disintegration. Plain repeats become bare copies under a stack label only
  the target carries; a wildcard stack keeps its label. With no label free,
  the join is refused with `no_free_group`.
- **A cluster target keeps its members' stacks.** The source joins as a ×1
  member: Disintegration ×2 onto `{Frost ×2 | Lightning ×2}` gives `{Frost
  ×2 | Lightning ×2 | Disintegration}` and `Disintegration`.
- **A combined level never travels into a cluster.** A joined ring drops
  its combined level; the rings its stack leaves behind keep counting, their
  total capped at what they can still reach, or stop when only one is left —
  the sheet's rule for a count stepped down. Ring of Energy ×3 `Σ ≥ 11`
  joined onto Ring of Might leaves `Ring of Energy ≤2 Σ ≥ 8`. A combined
  level that is the target keeps its count as a member's stack and loses
  its `Σ`: Ring of Might onto Ring of Energy ×3 `Σ ≥ 11` gives `{Ring of
  Energy ×3 | Ring of Might}`.
- **Categories mix freely.** #190 refused a join across categories when
  either entry was a stack: a cluster's stack then had to name one kind for
  its copies, and "spear or wand" names none. Every copy now keeps its own
  chip's kind — `{Wand of Frost ×2 | Plate Armor}` is two Frosts or the
  armor — so no join needs a stack spanning kinds, and none is refused for
  it. The `mixed_category_stack` refusal is gone.

A join never orphans a copy: every entry but the two joined asks for what it
did. It adds a row only for the rest of a stack a member shared, and
removes none except where the rest of a member's stack becomes alike
another member's: the two then share one label and its copies, and the
other label's copies are deleted (the cluster asks for what it did —
`{Frost ×3 | Disintegration ×2}` losing a Frost gives `{Frost ×2 |
Disintegration ×2}` under one label). A detach does the same. It never
leaves a stack or combined-level label on a chip without copies, and writes
a canonical list. Detaching a lone source again folds it
back with the copies it left behind: Disintegration ×2 and Frost → join →
`{Frost | Disintegration}` and Disintegration → detach Disintegration →
Frost and Disintegration ×2.

A **detach** moves one item the same way: Frost out of `{Frost ×2 |
Disintegration}` gives `{Frost | Disintegration}` and Frost, the detached
Frost keeping its key and constraints and the one left in the cluster
taking its place. A ×1 member leaves its cluster, and a cluster of one
dissolves into a chip.

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
  | `blanket_total` | `A blanket requirement cannot count levels together.` |
  | `no_free_group` | `Every group label is in use. Remove a stack or a combined level first.` |

  `no_free_group` also answers an edit that needs a stack label for a
  cluster member's stack — a count or copy floor that sets it apart from
  the member it shared its stack with, a join onto a stacked lone chip, a
  detach or a removal of one item that leaves the rest of a shared stack
  with copies — when all four are taken.

### When to normalize

Every edit that changes the rows writes the canonical encoding, so a list
the editor wrote needs no `normalize`. A list from elsewhere may not be
canonical — a cluster of one, repeats a stack would fold, alike member
stacks under two labels, a stack labelled 7 — and the board draws it as it
is, its problems included.

- **Send it** once when a list comes into the editor from outside — a share
  link, a query or results file, a preset applied, a saved state restored —
  and write the rows back when `changed` is set.
- **Skip it** where rewriting the list would break a match with a stored
  copy of itself: a search that must resume, a preset matched by its
  fingerprint, a results file that must keep matching the query it was
  searched with. Where both apply, skip it. Nothing needs `normalize`
  there: the first edit normalizes the list anyway, and the problems still
  gate Start and Share, so a label out of range is reported rather than
  searched.

An edit that takes a row out of its entry or reshapes a member's stack —
`join`, `detach`, `remove`, `remove_one`, a member's count or copy floor, a
member's save — reads such a list in its canonical encoding first. A named
stack written as bare copies under a stack label then leaves its copies
what the board showed them to be (`Wand of Disintegration` copies stay
Disintegrations, not `Any wand`), and two alike member stacks under two
labels act as the one stack the board shows. The drop policy and the join
candidates still answer for the list as written, and agree with the edit.

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
| `focus` | The row to follow — scroll to, highlight, announce: the joined source, the detached row, the chip an edit reshaped or a save landed in (the chip a saved plain repeat folded into), as the last edit that applied left it. A save names its chip even when it stored what was already there (`changed: false`), so a closing sheet can return to it. `null` after a removal (`remove`, `remove_item`, a `remove_one` that took the chip itself) and when no edit applied. Always a visible row. |
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
| `name` | The entry's name where a menu or a drag caption names it — an "Either/or with…" choice: a chip's `name`, or a cluster's members' names joined with ` or ` (`Spear or Mace`). |
| `members` | The visible rows' keys: one for a chip, every member of a cluster. |
| `extras` | Every hidden copy's key, in list order: the copies behind all its chips' badges, each once (members whose stacks are alike share theirs). |
| `chips` | One CHIP per member, in member order. The entry has no badge or stepper of its own: they are its chips'. |
| `problem` | The first problem touching any member or hidden copy, so a problem on a folded-away copy still shows. |

**CHIP**, one visible row:

| Field | Meaning |
| --- | --- |
| `key` | The row's key (0 in a sheet preview). |
| `name` | The short name beside the sprite: the item (`Ring of Might`), or `Any weapon`, `Any melee`, `Any thrown`, `Any armor`, `Any wand`, `Any ring`, `Trinket`, `Artifact`. |
| `title` | The full title the popover and sheet lead with: the item, or `Any Tier 3+ melee weapon`. |
| `item` | The item's stable id, or `null` for a wildcard. |
| `kind`, `family` | The kind (`melee_weapon`) and family (`weapon`) for the sprite. |
| `tags` | Qualifiers after the name, in order: `Transmute ≤3` or `choose at +3`, the tier (wildcards only: `T3`, `T3+`, `T≤3`), the upgrade (`+3`, `+3↑`, style `upgrade`), `F≤9`. Each TAG is `{"text", "style": "plain" \| "upgrade" \| "credit", "tooltip"}`, `tooltip` the tag's own hover text or `null` (a chip's tags have none; see RESIN_CHIP). |
| `trailing_tags` | Qualifiers after the effect cue: `No resin`. |
| `effect` | `{"label", "effects", "any_enchantment", "curses_only"}` — `any enchantment` (`any glyph` on armor), one effect's name, or `effect: A/B/C`; `effects` in catalog order, the full set for any enchantment. `null` for any effect. |
| `uncursed` | Cursed items are ruled out (drawn as a check mark). |
| `details` | The popover's detail line as parts: `within 3 transmutations`, `choose at +3`, the upgrade (`exactly +3`, `+3 or higher`, `any upgrade` — left out while counting levels and on trinkets and artifacts), the effect, `uncursed`, `excluded from Auto resin`, the source (`Locked chest`), `floors 1–9`. |
| `relations` | The popover's relation lines, each `{"glyph": "or" \| "sum" \| "times", "text"}`: the cluster's other members, `up to 2 — levels add to ≥ 5` (with `; the extra copies: floors 1–20` when the copies' floor limits differ from the anchor's), `3 of the same kind — the extra copies: any upgrade, floors 1–4`. |
| `description` | The accessibility label: the title, then the details. It leaves out the relation lines and the badges, which the apps draw as nodes of their own with their own words (the cluster's `label`, each badge's `tooltip`); an app whose chip is one accessibility node appends `relations` itself. |
| `problem` | The row's own first problem, else the first problem between rows blaming it; a chip also speaks for its own hidden copies (every member sharing a stack for the copies they share). |
| `badges` | The badges the chip shows at rest: `count` when it asks for more than one item, `total` when it counts levels; each `{"text", "compact_text", "tooltip"}` (`×3` / `3 of the same kind`; `Σ ≥ 5`, compact `Σ≥5`, `Levels add to at least 5 (a +0 item counts 1)`), else `null`. A cluster member's badges are its own and are drawn on its chip, inside the cluster's outline. |
| `copies` | The keys of the hidden copies behind the chip's badge, in list order; members whose stacks are alike share theirs. |
| `stack` | What the chip's count, combined-level and copy-floor steppers offer: `count`, `max` (3), `can_grow`, `can_change_count`, `count_max` (the count stepper's upper bound: `max` while the chip can grow, else its `count`, which it may only shed copies from; never above `max`), `total`, `can_count_levels` (a lone named ring stack only), `level_capacity`, `default_total`, `copy_depth`, `can_set_copy_depth`, `count_text` (`×2`, or `≤2` while counting levels — present even at ×1 for steppers), `total_text` (`Σ ≥ 5`, `Σ ≥ 0` without a total). |
| `in_cluster`, `can_detach` | A cluster member, which "On its own" (`detach`) applies to. |
| `join` | The visible rows this chip may join, in list order — what "Either/or with…" menus, pick mode, accessibility actions and drag hover read. |
| `refuse` | The visible rows a join onto is refused, each `{"key", "reason", "message"}`, for hover feedback. |

Drops are decided from the chips: onto a row or cluster, `join` it when the
target is in `join`, show the message when it is in `refuse`, else do
nothing; onto the empty board of the chip's own section, `detach` a cluster
member (`can_detach`) and leave a lone chip where it is; onto the remove
target, `remove_one`. Every drag moves one item, so the chip being dragged
is drawn as that one item: its name and tags, without its badges. A detach
or a `remove_one` that needs a stack label when none is free is refused
like a join; the typed `drop_action` answers it, the envelope's `refused`
says so after the fact.

**RESIN_CHIP**: `{"name": "Arcane Resin", "tags", "uncursed", "tooltip",
"details", "description"}` — tags `Auto` or `≥N`, then `Mage +2`, then
`F≤N`. The amount tag is always there and always first, so a summary may
read `tags[0].text` (Android's collapsed header: `Auto Arcane Resin`, `≥4
Arcane Resin`). The first two are the resin the chip counts, style
`credit`, which apps tint apart from the donor filter `F≤N` (`plain`); the
amount tag's `tooltip` explains Auto (`Enough resin to upgrade kept wands
to +3, excluding No resin wands and reforge copies`, `null` for a fixed
amount), and `Mage +2`'s says where it comes from (`Starting Magic Missile
contributes 2 resin`). The chip's own `tooltip` is its hover text: the
donors' source (`Locked chest`), the one filter no tag shows, or `null` for
any source.

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

- `open` — on the row `key` (a hidden copy's key opens the chip whose copy
  it is), or on a new chip with `key: null` (or a key not in the list,
  which the new row will take); `blanket` picks the new chip's section;
  `resin` is the query's current resin condition, which seeds the resin
  section; `offer_resin` offers Arcane Resin among the wands; `open_resin`
  opens the query's resin chip (`mode: "edit"`, origin `resin`) — or, when `resin` is
  `null` because the query has none, a new sheet with Arcane Resin picked
  (`mode: "new"`, origin `new`), so the chrome says Add and offers no
  Remove. Everything but `rows` defaults to `null`/`false`, and `null` means
  the default.
- `change` — applies one control the user moved.
- `save` — saves the draft onto `rows`, the list as it is now. When the row
  the sheet was opened on has since become a hidden copy of another chip,
  the save adds a new chip rather than vanishing into the copy. A sheet
  saved untouched — the draft still says what a sheet opened on the row now
  would — writes nothing: the rows come back as they were, `changed:
  false`, with `focus` on the chip, even where the row holds what no control
  can show ("+0 or higher", a floor limit on an empty boss floor, a combined
  level out of reach, copies with floor limits of their own). The resin
  chip's sheet saved untouched likewise answers `resin: null`, so the
  query's resin keeps a floor limit on an empty boss floor. Two exceptions:
  - a save that **repairs** the list — takes a problem away and adds none
    (a hand-written floor limit beyond the dungeon, a field the row's family
    or section cannot carry, which the sheet opened without) — writes the
    repaired row, and a resin floor limit beyond the dungeon is saved back
    into it;
  - an untouched sheet is refused, as any save is, while its draft has
    errors: a problem of the row's that a control the sheet shows can fix
    (an item of another category, an uncursed curse), or a trinket the list
    already names twice (`This trinket is already required. …` — the one
    place a duplicate trinket is explained, since the board reports none).

Open and change answer `{"draft": DRAFT, "form": FORM}`. A save answers

```json
{"saved": {"rows": [ROW], "next_key": 9, "changed": true, "rekeyed": [], "focus": 8,
           "resin": {"set": RESIN} | {"clear": true} | null}}
```

or, when the draft cannot be saved, `{"draft", "form"}` again with the
reasons in `form.errors`. `resin` is `{"set": RESIN}` when Arcane Resin was
the picked item (the wand chip the sheet was opened on is removed from the
rows), `{"clear": true}` when the resin chip was saved as a requirement, and
`null` otherwise — the query's resin stays as it is, which includes the
resin chip saved untouched.

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

A change to a control the form hides — a tier on a named item, a blanket's
stack, a cluster member's combined level — changes nothing. Values are clamped into range; floor sliders
step over the empty boss floors (a single step up onto 5, 10 or 15
continues to 6, 11 or 16; every other move snaps down).

**FORM** is everything the sheet shows, words included: every option, value
and error, the labels of the check boxes, switches and steppers, the
section labels of the effect, the stack and the resin, the help texts, and
the resin section's choice and bounds. The headings above the pickers and
the mode pickers (`Category`, `Item`, `Tier`, `Upgrade`, `Source`), card
titles (`Item`, `Upgrade level`, `Stack`), the heading of a range toggle's
stepper or slider (`Maximum transmutations`, `Levels reach`) and a slider's
accessible name are dialog chrome, the app's own like the sheet's title and
buttons. Every control is filled whether it shows or not — its label, its
options, its value in words, its help text — so an app may read a hidden
one. A sheet opened on a cluster member shows the member's own stack — its
count and its copies' floor — as a lone chip's sheet does; only the
combined level stays hidden there, since it cannot sit in a cluster.
Every numeric control
carries `min ≤ value ≤ max`, even while hidden, and every picker option is
`{"value", "label", "group", "hidden"}` — `hidden` marks a choice offered
only because the draft already names it (a tier-1 item from an imported
query).

A range control carries its current value's label alone, not one per value:
apps draw it as a slider or a stepper, not as a labelled menu. A mode
picker's `value_label` is the value (`Tier 3 or higher`, `+2 or higher`),
which an app shows beside the slider; a floor toggle's is the whole reading
(`Within first 4 floors`, `Copies within first 4 floors`), as is a range
toggle's (`At most 3`, `≥ 5 across up to 2`). The slider's accessible name
is the app's — fixed (the web's `Within first`) or following the mode (the
web's `Minimum tier`, Android's `At least`) — and it reads its value out
through `value_label`.

A help text shows with what it explains: a check box's `caption` under the
box whenever the box shows; the effect's under the "Specific…" grid
(`choices_visible`); the resin section's in the amount field's place while
`auto` is on; a range toggle's while its `caption_visible` is set — while
the switch is on for the transmutation limit, which it describes, and
whenever the control shows for the combined level, whose switch it
explains.

| Field | Meaning |
| --- | --- |
| `v`, `mode` (`new` \| `edit`), `origin` (`{"type": "new"}`, `{"type": "row", "key": K}`, `{"type": "resin"}`), `blanket`, `in_cluster`, `resin_picked` | What the dialog chrome — title and button labels, which apps own — derives from. |
| `title` | The sheet header's title: the requirement's (`Any Tier 3+ melee weapon`, `Rat Skull`), or `Arcane Resin` while the resin is picked. Unlike `preview` it is there while the draft has errors; the sprite follows `item` and `kind`. |
| `preview` | The CHIP a save would produce, with its stack and badges (key 0, no copy keys, no join candidates), or `null` while there are errors or the resin is picked. |
| `category`, `kind`, `weapon_type`, `item`, `source` | Pickers: `{"visible", "value", "options"}`. `item` lists the wildcard (`Any melee weapon`) unless the family always names one, `Arcane Resin` when offered, then the items — weapons grouped `Tier 2`…`Tier 5`. |
| `tier`, `upgrade` | `{"visible", "mode", "modes", "value_visible", "value", "min", "max", "value_label"}` (`Tier 3 or higher`, `+2 or higher`); `value_visible` says the value slider shows: the control does, in a mode other than `any`. |
| `effect` | `{"visible", "label", "mode", "modes", "choices_visible", "choices", "groups", "caption"}`; `label` is the section's (`Enchantment`, `Glyph` on armor), `choices_visible` says the "Specific…" grid shows (the control does, in mode `specific`), each choice `{"value", "label", "group": "enchantment" \| "curse", "selected"}`, curses listed only while the item may be cursed. |
| `uncursed`, `exclude_resin`, `select_trinket` | Check boxes: `{"visible", "value", "label", "caption"}`, `caption` the help text under the box or `null`. `exclude_resin` and `select_trinket` have one (`Keep this wand without budgeting resin to upgrade it. …`, `Applies after the first brewing opportunity. …`). |
| `floor_limit` | `{"visible", "enabled", "value", "options", "label", "value_label"}`; options skip the empty boss floors. |
| `transmutations` | A range toggle: `{"visible", "enabled", "value", "min", "max", "label", "caption", "caption_visible", "value_label"}`; the stepper shows while `enabled`, the caption while `caption_visible`. |
| `stack` | `{"visible", "label", "count", "min", "max", "value_label", "copy_depth", "count_levels"}`: `label` `Total item count`, `copy_depth` a floor toggle, `count_levels` a range toggle (`≥ 5 across up to 2`, caption `Each item counts its upgrade plus one, and spare items may go unused.`, shown whenever the toggle is; never on a cluster member). It shows for a cluster member as for a lone chip: the member's own stack. The section has no caption: only Linux ever gave it one. |
| `resin` | `{"visible", "label", "auto", "modes", "caption", "amount", "min", "max", "include_mage_wand"}`: `label` `Minimum resin`, which the amount field takes too; `modes` the Amount/Auto choice, each option valued as `auto` is (`false` `Amount`, `true` `Auto`); `caption` what Auto means, shown in the amount field's place while `auto` is on; `amount` the number as typed; `min`, `max` the amounts that save, the query format's 1–65535; `include_mage_wand` a check box (`Include Mage’s starting wand`, with its help text). |
| `errors`, `can_save` | Why the draft cannot be saved, in the order to show them. |

The sheet keeps every field of the row, shown or not. It re-encodes only
what its controls cannot hold — a stored "+0 or higher" opens as any
upgrade, "+max or higher" as exactly +max, a floor limit on an empty boss
floor as the floor below (one beyond the dungeon as floor 24), a total out
of reach as the stack's capacity, hidden copies with floor limits of their
own as the first copy's (one copy floor for them all) — and drops only
what the row's family or section cannot carry, under a control the sheet
hides for it: a trinket's tier or effect, a tier on a named item, an
effect off weapons and armor or of the other family, resin exclusion off
an ordinary wand, trinket selection off an ordinary trinket, another
family's transmutations or melee/thrown narrowing.
Fields no control shows are kept through every save: an artifact's upgrade
(the query format accepts +1…+5 — the city vault transfers +5 into its
artifact — but no app ever offered a control for it) and a trinket's
source, floor limit and uncursed filter. A category switch resets them, as
it resets everything the new family does not share.

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
| `board_items`, `BoardItem`, `ChipStack`, `ItemKey`, `join_candidates`, `drop_action`, `DropTarget`, `DropAction` | The fold itself — every entry's members, each with its `ChipStack` (its copies and combined level) — and the drop policy (`DropAction::RemoveOne` for the remove target). |
| `problems(rows)`, `row_problems(requirement)`, `Problem`, `ProblemScope` | The problem list. |
| `open`, `change`, `form`, `save`, `Draft`, `Change`, `Form`, `SaveResult`, `ResinOutcome`, `ResinState`, `ResinAmount` | The sheet. |
| `can_grow`, `can_change_count`, `count_max`, `can_count_levels`, `level_capacity`, `default_total`, `copy_depth`, `can_set_copy_depth`, `stack_view` | Stack rules, each of one chip's `ChipStack`. |
| `skip_boss_floor`, `compact_alternative_labels`, `labels` | Floor-slider stepping, label compaction, and every English phrase. |
| `requirement_board`, `requirement_editor` (feature `json-query`) | The envelopes. |

## Settled behaviour

The apps disagreed on these before the rules moved to the core; each is now
decided once.

| Topic | Behaviour |
| --- | --- |
| Badges and steppers | Every ×N and Σ badge and every count stepper is a chip's — a lone chip's or a cluster member's; a cluster draws none of its own. Members whose stacks are alike share one label and each shows ×N. |
| Joining across categories | Allowed, with or without stacks: every copy keeps its own chip's kind (#190's refusal is lifted); leftover labels on chips without copies are dropped, and no row is deleted but the copies of member stacks the join makes alike, which merge under one label. |
| Joining a stack | One item moves (drag, pick mode, menu and accessibility alike): the source's own row, the rest of its stack staying where it was, one fewer, with its own floor limits; a stacked lone target keeps its stack as a member; a target cluster's members keep theirs, and the source joins as ×1. See [Joins](#joins). |
| A combined level losing a ring to a join | The rings left behind keep counting, capped at what they can still reach, or stop when one is left. A counting target keeps its count as a member's stack and drops its Σ. |
| Detach, and a member dragged out of a group | One item moves: the member's row, with its constraints; the rest of its stack stays in the group in its place, one fewer; a ×1 member leaves, and a group of one dissolves into a chip. |
| Remove | The remove target takes one item (`remove_one`); the chip menu's "Remove" takes the chip with its whole stack (`remove`). |
| A member stepped down to ×1 | Drops its stack label, so it uses none of the four and none reaches a saved or shared query. |
| Counting levels on or off | The anchor and every copy keep their own floor limits, on the board and through the sheet (which saves the copy floor its hidden control holds); a counting stack grown on the board gives the new copies the copies' floor; the popover names the copies' floors while they differ from the anchor's. |
| A stacked cluster member saved into another category | Saved, with its stack rebuilt in its new kind (or dropped when it can no longer stack); the other members keep theirs. |
| Drop on the empty board | Detaches cluster members only; a lone chip stays. |
| Combined level on a blanket | Refused. |
| A member's stack label | Its own stack's, never spread onto other members; trinkets, artifacts and blankets never carry one. |
| Copy contents | Built from defaults; plain copies keep the melee/thrown narrowing; resin exclusion, blanket, trinket selection and transmutations are never copied. |
| Key lookup | Visible members only, never hidden copies. |
| Labels out of range | Moved onto free labels in range by `normalize` and every edit that changes the rows; never merged to fit. |
| Saving an unchanged chip | Writes nothing: identical rows, the same copy keys, `changed: false`, even for values no control can show; the resin chip answers `resin: null`. A save that repairs a hand-written row's problem writes the repair. |
| Fields the sheet does not show | Kept through every save (an artifact's upgrade; a trinket's source, floor limit and uncursed filter); a category switch resets them. |
| Fields a row cannot carry | A hand-written field the row's family or section cannot carry, under a control the sheet hides (a blanket wand's resin exclusion), is dropped on open, so the sheet can save the row. |
| New rows | Appended; sections are never reordered. |
| Wildcard chip names | `Any melee`, `Any thrown`. |
| Titles | `Any Tier 3 weapon`, `Any Tier 3+ weapon`, `Any Tier 3 or lower weapon`. |
| Item names | Title case, as in the catalog asset. |
| Source labels | Sentence case (`Locked chest`). |
| Tag order | Transmutations or `choose at +3`, tier, upgrade, floor, then the effect cue, then `No resin`. |
| Combined-level badge | `Σ ≥ T`, compact `Σ≥T`. |
| Resin chip tags | The amount and `Mage +2` styled `credit`, apart from `F≤N`; `Auto` and `Mage +2` carry their own tooltips. |
| Entry names in menus | A chip's name; a cluster's member names joined with ` or ` (Linux and macOS). |
| Armor's any-effect | `any glyph`. |
| Default combined level | The item count, within the capacity. |
| Level capacity | The stack's ring capacity, or its group's attainable capacity once counting. |
| New chip | Any weapon; a new blanket takes the first ordinary row's kind and melee/thrown narrowing. |
| Category switch | Keeps source, floor and uncursed where the new family has them; resets the rest. |
| Default tier | 3. |
| Upgrade bounds | Exactly +1…max, at least +1…max−1; a stored "+0 or higher" opens as any, "+max or higher" as exactly max, and is written so only by a save that changes something. |
| Resin sheet on a query without resin | A new sheet with Arcane Resin picked (`mode: "new"`): Add, no Remove. |
| "Specific…" with nothing ticked | Allowed; saves as any effect. |
| Floor pickers | Skip the empty boss floors. |
| Artifact transmutations | Supported, 1–10. |
| Save guard | A save that newly breaks the list around the saved row is refused. |
| Resin section | Seeded from the query's resin (uncursed donors by default), kept apart from the wand draft. |
| Dialog chrome | App-owned: sheet titles, card titles, button labels, the headings of the pickers and mode pickers and of a range toggle's stepper or slider, slider accessible names. The labels of check boxes, switches and steppers, the effect, stack and resin section labels, the help texts and the resin section's words are the form's. |
| Board chrome | App-owned: menu items and their headings (`Edit…`, `Remove`, `Either/or with…`, `How many`, `Combined level`, `Count levels together` / `Stop counting levels`, `On its own`), drag captions, and the hover text of a glyph such as the uncursed check mark. The words on the board itself — chip names, tags, details, badges, entry names, problems, refusals — are the core's. |
| Help texts | A check box's always under it; the transmutation limit's while it is on; the combined level's beside its switch (`caption_visible`). |
| Chip problems | The row's own, then cross-row blame; hidden copies surface on their chip (every member sharing them) and on their entry. |

### Known limitations

These are the core's own choices:

- The sheet has no control for an artifact's upgrade or a trinket's source,
  floor limit and uncursed filter — no app ever offered one. It keeps them,
  and a category switch resets them, but they cannot be edited on their
  own.
- A hand-written list with more than four stacks (or combined levels) keeps
  the labels of those no label is left for, and they block Start and Share
  until a stack goes.
- An untouched save repairs a hand-written problem only where the board's
  `save` rewrites the row. One it reads as the entry's own shape — a lone
  blanket carrying a stack label, a stack whose copies disagree on the
  combined level — stays unless the row needs rewriting anyway;
  `normalize` repairs the first, a save that changes the stack the second.
- The board reports no duplicate trinket; only the sheet does, as an
  error, so an untouched sheet on either of two rows naming one trinket
  cannot be saved.
- A new sheet that picks Arcane Resin among the wands on a query that
  already has resin stays `mode: "new"`, although its save replaces that
  resin (`set`). Apps that offer the pick decide Save and Remove from the
  query's resin themselves, or open the resin chip's sheet instead.
- The copy floor has no help text; the one Android and iOS drew ("A floor
  limit is where an item lies, not what it is, so the copies keep their
  own.") is gone until the floor toggle carries a caption.
- The board's count stepper leaves a combined level a stack stepped down
  can no longer reach as it was, and the problem list reports it; the
  sheet's stepper, `remove_one` and a join cap it.
- A join onto a stacked lone chip without a stack label (plain repeats, a
  combined level) takes a free label before its source gives any back, so
  with all four in use it is refused even where the source's own stack
  would have freed one.
- A hand-written list that ties a lone chip to the members of a cluster
  with one stack label ("the cluster's slot holds the chip's item") shows a
  chip and a cluster without the tie: the board has no shape for it, and
  the edits carry the label through.
- The copy floor of a stack counting levels cannot be edited until it
  stops counting; the sheet keeps it, hidden, and the popover names it.

## Golden fixtures

`crates/seedfinder-core/tests/fixtures/editor/*.json` pins representative
request/response pairs for both envelopes: the board tour, the four stack
encodings, a member's own stack and alike member stacks drawn as each
member ×N, joins (one copy moving out of a stack, the reported list and
its round trip, onto a stacked chip, a stacked cluster and a combined
level, a combined level losing a ring, a member moving one item, across
categories, a hand-written stack, refused for want of a label), detaches
(a member alone, one copy of a member's stack) and removals (one member, a
whole stack, one item of a member's stack and of a lone combined level),
copy floors, combined levels (their copies' floors kept both ways and
given to new copies), a member stepped down to ×1, saves (new and
unchanged), problems, key repair, label compaction and labels moved into
range, unreadable rows (and the labels they hold, which no join takes), the sheet's open/change/save flow, a member's sheet
and its save, an untouched save and one that repairs its row, the resin
flows (a query with resin and one without, and the resin chip saved
untouched), and the error envelopes.
Each file is

```json
{"about": "...", "envelope": "requirement_board", "request": {...}, "response": {...}}
```

A request is sent as its JSON text — except a request stored as a string,
which is sent as that string (the fixtures for text that is not JSON at
all). Sheet requests carry the very draft string an earlier response
returned. App tests replay the files through their own binding and compare
the answers.

The core test `tests/editor_fixtures.rs` regenerates every pair and fails
when a file drifts; it also opens a sheet on every row the fixtures send
and checks that saving it untouched answers what the board answers for the
rows alone, or repairs them. After a deliberate change, review the answers
and rewrite the files:

```sh
UPDATE_EDITOR_FIXTURES=1 cargo test -p shpd-seedfinder-core --test editor_fixtures
```
