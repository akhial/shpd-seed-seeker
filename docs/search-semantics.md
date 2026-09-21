# Search semantics: one action and a retained seed pool

Web, Android, Windows, macOS, and Linux have one **Search** action.

1. Search re-verifies every seed in the saved pool against the current query.
2. Matching seeds become the current results view.
3. Search continues looking for matches, counting the verified survivors toward
   the 1,024-result goal. If the current matches already fill the list, another
   Search asks for another batch. Cancellation, an impossible query, or an
   exhausted traversal can finish below the goal.

Every loaded or discovered seed remains in the pool until the user chooses
**Clear results**. Changing requirements, a zero-match filter, cancelling,
failed scans, importing another file, and the 1,024-row display limit never
remove saved seeds. New imports add to the pool and become the current view.
Previously hidden seeds can reappear when any later query matches them.

The pool is uncapped; only the current results view is capped. Exports describe
that view and its query. Existing result-file decoding limits still apply to
new imports. Android saves the pool in its private checkpoint; the other
frontends retain it for the current app session.

## Scanning and coverage

A completed or cancelled traversal records its query and exact remaining range.
An unchanged query can resume that range after checking the entire pool again.
An edited query starts a fresh traversal after checking the pool. Imports have
no scan cursor. No item-overlap, containment, related/unrelated, anchor, or
separate-search classification is used. The old decision APIs are removed.

Native accept limits include rediscovered seeds. Frontends deduplicate matches
and continue capped batches until enough unique current matches arrive, the
traversal ends, or the user cancels. Failed scans retain delivered seeds but do
not create new reusable coverage.

## Original trinket choices

Each pool entry retains its original recipe and source query. Entries from
different searches or imports can coexist. Verification groups seeds by their
source query and uses the shared engine matcher:

- A saved non-null recipe keeps that trinket.
- With automatic selection enabled, a null recipe can represent a choice
  removed as unnecessary. The engine reconstructs the original choice from
  the source query and reapplies it **before testing** the current query.
- A failure in the chosen world is accepted; it is not retried in a plain world.
  Successful matches receive the normal cleanup that removes an unnecessary
  trinket when the plain world also matches.

New scans use the current query's automatic selection rules. Their discoveries
retain that query as their own source; they do not overwrite earlier pool
entries or their original choices. Query relationships are irrelevant to
verification and retention.

Android checkpoints preserve sources and pending work. Older checkpoints keep
their known seeds and discard traversal assumptions from the removed routing
model. A pending search restarts verification and scanning under this model.

The results-file schema remains unchanged. Exporting current matches and
reimporting them does not carry the private per-seed source queries; imported
null recipes use the exported query as their source.
