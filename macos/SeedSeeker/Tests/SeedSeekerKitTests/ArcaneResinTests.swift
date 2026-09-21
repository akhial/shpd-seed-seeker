import Foundation
import XCTest
@testable import SeedSeekerKit

final class ArcaneResinTests: XCTestCase {
    func testResinOnlyQueriesPreserveAmountsAndFiltersAcrossFormats() throws {
        for amount in [0, 1, 3, 65535] {
            for filter in [ArcaneResinFilter(), ArcaneResinFilter(uncursed: false, maximumDepth: 12, source: .wandmakerReward)] {
                let query = SavedQuery(autoApplyTrinket: false, arcaneResin: amount, arcaneResinFilter: filter, arcaneResinAuto: amount == 0)
                let request = try query.searchRequest()
                XCTAssertEqual(request.slotCount, 1)
                XCTAssertNotNil(query.validated())
                let persisted = try JSONDecoder().decode(SavedQuery.self, from: JSONEncoder().encode(query))
                let requestCopy = try JSONDecoder().decode(SearchRequest.self, from: JSONEncoder().encode(request))
                XCTAssertEqual(try QueryDocument.encode(requestCopy), try QueryDocument.encode(request))
                let linked = try DeepLink.decode(DeepLink.encodeLink(for: query))
                let decoded = try ResultsExport.decodeQuery(ResultsExport.encodeQuery(query))
                let exported = try ResultsExport.decode(ResultsExport.encode(query, seeds: ["AAA-AAA-AAA"], appVersion: "test"))
                for restored in [persisted, linked, decoded, exported.query] { XCTAssertEqual(restored, query) }
            }
        }
        XCTAssertThrowsError(try SearchRequest(requirements: []))
        for amount in [-1, 65536] { XCTAssertThrowsError(try SearchRequest(requirements: [], arcaneResin: amount)) }
        XCTAssertThrowsError(try SearchRequest(requirements: [], arcaneResin: 2, arcaneResinFilter: .init(maximumDepth: 25)))
        XCTAssertEqual(try ResultsExport.decodeQuery(["requirements": [], "arcane_resin": 6]).arcaneResinFilter, .init())
    }

    func testNativeResinScoutProbabilityAndRefinement() async throws {
        let query = try SearchRequest(requirements: [], arcaneResin: 2)
        let world = try await ProductionSeedFinderEngine().scoutSeed("AAA-AAA-AAA", challenges: 0)
        let marks = try ScoutMatches.mark(seed: world.seed, challenges: 0, query: query)
        XCTAssertEqual(marks.totalRequirements, 1)
        XCTAssertEqual(marks.matchedRequirements, 1)
        XCTAssertFalse(marks.matched.isEmpty)
        for index in marks.matched { XCTAssertEqual(world.items[index].item.kind, .wand) }
        var harder = query; harder.arcaneResin = 65535
        XCTAssertEqual(try ScoutMatches.mark(seed: world.seed, challenges: 0, query: harder).matchedRequirements, 0)
        XCTAssertNotNil(try QueryAnalysis.analyze(QueryDocument.encode(query)).probability)
    }
    func testAutoBlanketSharesItsWitnessAndPreservesTheEngineEstimate() throws {
        let document = Data(#"{"arcane_resin":"auto","requirements":[{"item":"wand_lightning","upgrade":2},{"kind":"wand","upgrade":2,"blanket":true}]}"#.utf8)
        let saved = try ResultsExport.decodeQuery(JSONSerialization.jsonObject(with: document) as! [String: Any])
        XCTAssertTrue(saved.arcaneResinAuto)
        XCTAssertEqual(saved, try DeepLink.decode(DeepLink.encodeLink(for: saved)))
        let query = try saved.searchRequest()
        XCTAssertEqual(query.slotCount, 3)
        let marks = try ScoutMatches.mark(seed: "AAA-AAA-AAS", challenges: 0, query: query)
        XCTAssertEqual(marks.totalRequirements, 3)
        XCTAssertEqual(marks.matchedRequirements, 3)
        var direct = query; direct.requirements.removeAll { $0.blanket }
        let probability = try XCTUnwrap(QueryAnalysis.analyze(QueryDocument.encode(query)).probability)
        let directProbability = try XCTUnwrap(QueryAnalysis.analyze(QueryDocument.encode(direct)).probability)
        XCTAssertEqual(probability, directProbability, accuracy: 1e-12)
        XCTAssertThrowsError(try SearchRequest(requirements: query.requirements.filter { $0.blanket }, arcaneResinAuto: true))
    }

}
