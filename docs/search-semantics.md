# Search semantics: the Target Set

Web, Android, Windows, macOS, and Linux share the Target Set lifecycle and
expose separate Search and Filter loaded seeds actions, described below. The model exists so that
a user's found seeds are never thrown away by starting another search: the
only action that discards results is the explicit **Clear** button.

## Definitions

- **Target Query** — the query of the first search after boot (or after
  Clear, or the query embedded in an imported results file). It stays fixed
  until Clear or a new import.
- **Target Set** — every unique match the Target Query's traversal has
  delivered, uncapped (the display cap only limits what is listed). It grows
  when an extending search scans previously uncovered range; it never
  shrinks. The *displayed* list is capped at 1,024 rows on every platform —
  a run's full result set (survivors plus new finds) can exceed that, and
  the excess must still reach the Target Set and any later refine, just not
  the screen.
- **Target coverage** — the portion of the seed space the target traversal
  has scanned, kept as resume state (uncovered remainder). Imported results
  carry no coverage.
- **World conditions** — the query-level conditions that judge an otherwise
  unchanged world: require an accessible blacksmith, exclude the Smith
  rewards, and demand a Wandmaker quest. Switching one on, or naming a quest
  where none was demanded, only ever *removes* seeds, so one query's
  conditions can be **at least as strict** as another's: every flag the base
  sets is also set, and a base quest is demanded unchanged (an unfiltered base
  accepts any). The floor limit and challenges are not world
  conditions — they change which world is generated.
- **Continuation**: query B continues query A when the floor limit and
  challenge set are identical, B's world conditions are at least as
  strict as A's, and every
  slot of A is covered by a *distinct* slot of B at least as strict — a slot
  being one requirement, or one "any of these" group of alternatives — where
  B's slot is at least as strict when each of its members implies some
  member of A's: equal, or strengthened by an item named where A wanted any
  of its kind, a tightened upgrade/tier bound, a demanded source, a narrowed
  effect set, curse state, per-item floor limit, or a dropped alternative.
  (A plain requirement multiset superset is the special case where the
  covering requirements are equal.) A combined-level group of A must be
  carried over intact: the slots covering its members must form exactly one
  group of B, of the same size, with at least A's total — and since a
  combined-level member is optional (any subset of its group may carry the
  total), it can never cover a plain slot of A. Only then is
  every B-match inside A's covered region already in A's matches, which is
  what makes filter-and-resume sound; loosening any requirement, adding an
  alternative, or lowering a total breaks the containment and B must rescan.
  The engine owns this
  predicate under preserved selection — `SearchQuery::refines` in `seedfinder-core`, exposed as
  `seedfinder_query_continues` (C), `JniBindings.queryContinues` (Android)
  and `query_continues` (wasm) — and frontends should call it rather than
  re-derive it.
- **Arcane Resin** — `arcane_resin` is a minimum yield from surplus wands
  within the query's floor limit. Donors must be uncursed by default;
  `arcane_resin_filter` can allow cursed wands or narrow their floor and source.
  Each generated wand contributes `2 * (upgrade + 1)`, without resin upgrades
  or hero talent bonuses. The engine reserves items for the ordinary slots and
  chooses compatible surplus wands, respecting all reward choices and excluded
  sources. Resin counts as one scout condition, highlighting its contributing
  wands. Raising the minimum or tightening donor filters may continue a query;
  loosening them requires re-filtering or rescanning. Resin searches share an
  item with other resin searches and wand searches. With `"arcane_resin":"auto"`,
  the minimum is calculated for each assignment: each reserved +0 wand needs 6,
  +1 needs 5, +2 needs 3, and +3 or higher needs none. Only wands actually
  assigned to ordinary slots contribute; an OR slot contributes its selected
  member, and a query with no assigned wands needs no resin. Auto remains one
  scout condition even when no donors are needed. Switching between Auto and
  a positive fixed amount requires re-filtering or rescanning. Auto can refine
  another Auto query under the same ordinary narrowing and donor-filter rules.
  JSON and share links preserve the minimum (version 7), donor filters
  (version 8), blankets (version 9), and Auto (version 10); existing queries
  retain their prior encoding. Web, Android, macOS, Windows, and Linux offer Resin in the Wand
  editor and support resin-only queries, as does the CLI's canonical query
  input. All five editors support both Amount and Auto modes. Probability estimates integrate wand upgrades and
  surplus resin together, reserving each item once and replacing an earlier
  reservation when a cheaper matching wand arrives. This includes mixed
  upgrades without enumerating donor plans. Blanket filters narrow only the
  reserved items before resin is calculated; witnesses add no extra upgrade
  cost, and donors cannot satisfy blankets. The model uses measured supply
  counts and keeps quest rewards mutually exclusive. It retains the ordinary
  equipment estimate's identity-deck and world-condition corrections, with
  the conditional resin chance calculated from the joint supply. Overlapping
  filters use a scarcest-first allocation; this remains an estimate rather
  than an exhaustive count of seeds. Pure supply calculations are cached per
  worker with bounded storage, including profile and all donor filters in the
  keys, so repeated trinket scoring does not repeat the supply calculation.
- **Shares an item**: some requirement of B and some requirement of A have
  the same kind, and either at least one of the two names no specific item or
  both name the same item. Scope and challenge differences are irrelevant
  here — a filter re-verifies seeds from scratch under B. The engine owns
  this predicate too — `SearchQuery::shares_item` in `seedfinder-core` — and
  frontends should call it rather than re-derive it.

## Search and Filter loaded seeds

**Search** first re-verifies saved results, then keeps looking for
matches. An added requirement, an unrelated item, or a changed automatic
trinket policy never prevents checking the saved seeds.

- When the new query continues the Target Query, Search filters the full
  Target Set and resumes its uncovered remainder. Imported results have no
  coverage: Search filters them and starts a fresh traversal. Known exhausted
  coverage remains exhausted. Checkpoints persist this distinction.
- When continuation cannot be proved, Search filters the Target Set and
  scans the new query from scratch, leaving the Target and its coverage
  unchanged. Repeating or narrowing that search can resume its own traversal.
  The engine's `SearchQuery::refines` owns the coverage proof;
  frontends do not derive it from item overlap.
- Search fills the displayed list toward 1,024 unique seeds, counting filter
  survivors toward that goal. Native batches that only rediscover saved
  seeds do not prematurely finish the search. If the list is already full,
  pressing Search asks for another batch, preserving the existing ability
  to grow the uncapped Target Set. Cancellation and exhausted coverage can
  end a search before the limit; worker draining can deliver extra matches.
- **Filter loaded seeds** only re-verifies the full Target Set against the
  current query. It accepts unrelated queries and never scans or changes
  the Target or its coverage. Applying query B to loaded results of A thus
  finds their intersection within that saved list. Subsequent filters use
  the full Target Set so dropped seeds can be recovered by loosening filters.

Adding requirements may change how a fresh search would rank trinkets. A
refinement keeps the original selection policy, so this change alone does not
break containment or cause an unrelated-query notice.

## Automatic start decision

The shared engine decides whether a query can refine the Target or continue
the previous detached scan. Target refinement takes priority; otherwise a
proven continuation of the last detached run takes priority over a mere item
overlap with the Target. For compatibility, the decision API still returns
`target-filter` for a shared-item relationship and `detached` for unrelated
queries. **Search** executes either as filter-then-fresh-scan; only the explicit
**Filter loaded seeds** action stops after filtering.

A target refine updates the uncapped Target Set with new finds and advances
coverage. A filter-only run or a detached scan leaves the original Target
unchanged. An unsatisfiable refine consumes no coverage, so removing the
impossible requirement can resume from the previous position.

With no Target (boot, after Clear, or after a failed first run), the search
is an *anchor scan*: a fresh full-range scan that, on completion or cancel,
establishes the Target Query, Target Set, and coverage. A run that fails
establishes nothing. If the Target Set is empty (an anchor that found 0),
a continuing `Q` still resumes its coverage, but any other `Q`
re-anchors instead — an empty set holds nothing worth preserving.

Displayed results are always genuine matches of the query that produced
them. After mixed refines the display is not necessarily *exhaustive* for
the covered range (a narrower query may have skipped seeds an earlier one
would have kept); the tool trades exhaustiveness for never losing results.

## Import, Clear, failure

- **Import** replaces everything: the imported query becomes the Target
  Query, the imported seeds the Target Set, with unknown coverage. Search
  starts a fresh traversal after filtering on every platform.
- **Clear** drops the display, the Target, and all coverage. It is the only
  way to do so.
- **Failure** leaves the Target as it was; a failed run is never a
  continuation base.

## Surfacing

The status surfaces introduced for refine progress (desktop footer/status
bar, mobile snackbar, GNOME toasts) also carry the target notes:

- Target refine / target filter reuse the existing "Verifying…", "Kept X of
  Y…", "Refined: kept X of Y" notes, with *Y = the Target Set size*.
- Search retains the survivors while looking for more matches. A changed
  trinket ranking does not produce an unrelated-query warning.

The result cap, stats box, chips, and impossible-query warning are
unchanged.

## Automatic trinket world conditions

`auto_apply_trinket` selects one initial offer before generation, based on a
query-wide ranking. Refinement freezes the original ranking instead of
recalculating it from the edited requirements. Explicit selection slots and
whether automatic selection is enabled must still agree for coverage reuse.
Changing these world settings requires a fresh traversal after filtering.

Saved non-null recipes keep their selected trinket. A saved null can mean that
the original automatic choice was removed as unnecessary. Refinement reapplies
that original choice **before testing** the new query. It does not predict
whether the seed will benefit. If the chosen world fails, that miss is accepted;
it does not try the plain world to rescue a failed search. Successful matches
receive the usual no-trinket replay: if that also matches, both world and recipe
are replaced with the no-trinket result. This keeps the initial search's accepted
tradeoff while allowing a previously unnecessary trinket to be used again.

Continued scans use the original choice rule with the new query's matcher and
pruning. Controllers retain that original query through successive refinements
and Android checkpoints. Search execution passes `{ "query": ..., "refine_base":
... }` to the engine; the envelope is validated and is separate from canonical
query documents, presets, and share links. Saved result recipes remain the
source for replaying non-null choices. Imports have no reusable coverage.

`SearchQuery::continues` still compares independently selected policies. The
public continuation bridges use `SearchQuery::refines`, which permits changed
rankings only because callers execute under the original policy. Passing that
verdict to an ordinary freshly ranked search would be incorrect.

The original selection query is session context, not part of the result-file
format. Exported non-null recipes replay exactly; a null recipe imported later
uses the exported query's policy when automatic selection is reapplied.
