// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation

public struct ScoutArtifactDeckEntry: Sendable, Identifiable {
    public let item: CatalogItem
    public let matched: Bool
    public let availableInDungeon: Bool
    public var id: String { item.id }

    public var accessibilityLabel: String {
        item.name + (availableInDungeon ? ", available in dungeon" : "")
            + (matched ? ", matches requirement" : "")
    }
}

extension ScoutWorld {
    /// The displayed order is captured before generation. Match indices refer
    /// to a remaining deck at a requirement's floor limit, so resolve those
    /// indices to identities before highlighting the full starting deck.
    public func startingArtifactDeck(matches: ScoutMatches?) -> [ScoutArtifactDeckEntry] {
        let natural = ScoutChoiceStatus.availableArtifactIDs(items: items, matched: matches?.matched ?? [])
        let targets = Set((matches?.transmutedArtifacts ?? [:]).flatMap { depth, indices in
            guard let snapshotDepth = artifactDecks.keys.filter({ $0 <= depth }).max(),
                  let remaining = artifactDecks[snapshotDepth] else { return [String]() }
            return indices.compactMap { remaining.indices.contains($0) ? remaining[$0].id : nil }
        })
        return (artifactDecks[0] ?? []).map {
            ScoutArtifactDeckEntry(item: $0, matched: targets.contains($0.id), availableInDungeon: natural.contains($0.id))
        }
    }
}
