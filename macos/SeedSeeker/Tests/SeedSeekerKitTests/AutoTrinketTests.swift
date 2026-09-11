import Foundation
import XCTest
@testable import SeedSeekerKit

final class AutoTrinketTests: XCTestCase {
    private func query() throws -> SavedQuery {
        try ResultsExport.decodeQuery(["max_depth": 19, "auto_apply_trinket": true,
            "requirements": [["item": "runic_blade", "upgrade": 1, "effect": "Grim"]]])
    }

    func testDefaultsAndExplicitNoneSurviveDocuments() throws {
        let saved = try query()
        XCTAssertTrue(SavedQuery(requirements: []).autoApplyTrinket)
        XCTAssertFalse(try ResultsExport.decodeQuery(["requirements": [["item": "runic_blade"]]]).autoApplyTrinket)
        XCTAssertTrue(try DeepLink.decode(DeepLink.encodeLink(for: saved)).autoApplyTrinket)
        let restored = try ResultsExport.decode(ResultsExport.encode(saved,
            seeds: ["SRU-YSU-QHS", "EYY-RUL-LQG"], appVersion: "test", trinkets: ["parchment_scrap", nil]))
        XCTAssertEqual(restored.trinkets, ["parchment_scrap", nil])
        XCTAssertTrue(restored.query.autoApplyTrinket)
    }

    func testNativeRefineStripsAndRestoresNeededTrinkets() async throws {
        let saved = try query(); let query = try saved.searchRequest()
        let engine = ProductionSeedFinderEngine()
        let matches = try await engine.filterRecipes(query, base: query, recipes: [
            SeedResult(seed: "SRU-YSU-QHS", matchedRequirements: 1, selectedTrinket: "parchment_scrap"),
            SeedResult(seed: "EYY-RUL-LQG", matchedRequirements: 1, selectedTrinket: "parchment_scrap"),
        ])
        XCTAssertEqual(matches.map(\.selectedTrinket), ["parchment_scrap", nil])
        let whip = try ItemRequirement(key: 2, item: XCTUnwrap(ItemCatalog.findById("whip")), upgrade: 0, effect: .oneOf(["Venomous"]), kind: .weapon, upgradeMatch: .any)
        let refined = try SearchRequest(requirements: query.requirements + [whip], maximumDepth: 19, autoApplyTrinket: true)
        let restored = try await engine.filterRecipes(refined, base: query, recipes: [matches[1]])
        XCTAssertEqual(restored.map(\.selectedTrinket), ["parchment_scrap"])
    }
}
