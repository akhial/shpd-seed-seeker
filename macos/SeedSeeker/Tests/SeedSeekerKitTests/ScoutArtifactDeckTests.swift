// SPDX-License-Identifier: GPL-3.0-or-later
import XCTest
@testable import SeedSeekerKit

final class ScoutArtifactDeckTests: XCTestCase, @unchecked Sendable {
    func testStartingDeckHighlightsIdentityFromTheRequirementFloorSnapshot() async throws {
        let world = try await ProductionSeedFinderEngine().scoutSeed("AAA-AAA-AAA", challenges: 0)
        let requirement = try ItemRequirement(key: 1,
            item: XCTUnwrap(ItemCatalog.findById("ethereal_chains")), upgrade: 0,
            kind: .artifact, upgradeMatch: .any, artifactTransmutations: 4)
        let query = try SearchRequest(requirements: [requirement], maximumDepth: 19)
        let matches = try ScoutMatches.mark(seed: world.seed, challenges: 0, query: query)

        let entries = world.startingArtifactDeck(matches: matches)
        XCTAssertEqual(entries.map(\.id), world.artifactDecks[0]?.map(\.id))
        XCTAssertEqual(entries.count, 11)
        XCTAssertEqual(matches.transmutedArtifacts, [19: [3]])
        XCTAssertEqual(entries[5].id, "ethereal_chains")
        XCTAssertTrue(entries[5].matched)
        XCTAssertFalse(entries[3].matched, "A remaining-deck index must not highlight the same position in the starting deck")
        XCTAssertEqual(entries.filter(\.matched).map(\.id), ["ethereal_chains"])
        XCTAssertEqual(Set(entries.filter(\.availableInDungeon).map(\.id)),
                       ["unstable_spellbook", "sandals_of_nature", "alchemists_toolkit", "skeleton_key"])
        XCTAssertTrue(world.startingArtifactDeck(matches: nil).allSatisfy { !$0.matched })
    }

    func testLegacyScoutWithoutDeckSnapshotsDoesNotInventAnOrder() {
        let world = ScoutWorld(seed: "AAA-AAA-AAA", items: [])
        XCTAssertTrue(world.startingArtifactDeck(matches: nil).isEmpty)
    }
}
