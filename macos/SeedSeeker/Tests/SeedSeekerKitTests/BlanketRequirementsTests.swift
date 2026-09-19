import Foundation
import XCTest
@testable import SeedSeekerKit

final class BlanketRequirementsTests: XCTestCase {
    private func wand(_ key: Int64, blanket: Bool = false) throws -> ItemRequirement {
        try ItemRequirement(key: key, item: ItemCatalog.findById("wand_frost"), upgrade: 0,
                            kind: .wand, upgradeMatch: .any, blanket: blanket)
    }

    func testBlanketsSurviveDocumentsLinksResultsAndSavedQueries() throws {
        var requirements = try ["wand_lightning", "wand_disintegration", "wand_frost"].enumerated().map { index, id in
            try ItemRequirement(key: Int64(index + 1), item: ItemCatalog.findById(id), upgrade: 2,
                                kind: .wand, upgradeMatch: .atLeast)
        }
        requirements.append(try ItemRequirement(key: 4, item: nil, upgrade: 3, kind: .wand,
                                               source: .wandmakerReward, blanket: true))
        let query = SavedQuery(requirements: requirements)
        let persisted = try JSONDecoder().decode(SavedQuery.self, from: JSONEncoder().encode(query))
        let restored = try ResultsExport.decodeQuery(ResultsExport.encodeQuery(query))
        let linked = try DeepLink.decode(DeepLink.encodeLink(for: query))
        let exported = try ResultsExport.decode(ResultsExport.encode(query, seeds: ["AAA-AAA-AAA"], appVersion: "test")).query
        for decoded in [persisted, restored, linked, exported] {
            XCTAssertEqual(decoded.requirements.map(\.blanket), [false, false, false, true])
            XCTAssertEqual(decoded.requirements.last?.source, .wandmakerReward)
            XCTAssertEqual(decoded.requirements.last?.upgrade, 3)
            XCTAssertNotNil(decoded.validated())
            XCTAssertNoThrow(try SearchRequest(requirements: decoded.requirements))
        }
    }

    func testBlanketsCannotStackOrJoinOrdinaryRequirements() throws {
        let requirements = try [wand(1), wand(2, blanket: true), wand(3, blanket: true)]
        XCTAssertEqual(requirements.boardCount, 3)
        XCTAssertFalse(requirements.canStack(requirements.boardItems()[1]))
        XCTAssertEqual(requirements.setStackCount(requirements.boardItems()[1], 3), requirements)
        XCTAssertEqual(requirements.joinAlternatives(source: 0, target: 1), requirements)
        let grouped = requirements.joinAlternatives(source: 1, target: 2)
        XCTAssertEqual(grouped.boardCount, 2)
        XCTAssertTrue(grouped.allSatisfy { $0.identityGroup == nil })
        XCTAssertNoThrow(try SearchRequest(requirements: grouped))
        let linked = try DeepLink.decode(DeepLink.encodeLink(for: SavedQuery(requirements: grouped)))
        XCTAssertEqual(linked.requirements.filter(\.blanket).count, 2)
        XCTAssertEqual(linked.requirements.slotCount, 2)
        XCTAssertEqual(grouped.detach(1).boardCount, 3)
    }

    func testInvalidBlanketsAreRejectedBeforeSearching() throws {
        XCTAssertThrowsError(try SearchRequest(requirements: [wand(1, blanket: true)]))
        var ordinary = try wand(1); ordinary.alternativeGroup = 1
        var blanket = try wand(2, blanket: true); blanket.alternativeGroup = 1
        XCTAssertThrowsError(try SearchRequest(requirements: [ordinary, blanket]))
        XCTAssertThrowsError(try ItemRequirement(key: 1, item: nil, upgrade: 0, kind: .wand,
                                               upgradeMatch: .any, identityGroup: 1, blanket: true))
    }
}
