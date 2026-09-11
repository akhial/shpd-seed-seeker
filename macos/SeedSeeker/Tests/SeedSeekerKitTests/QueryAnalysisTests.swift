import XCTest
@testable import SeedSeekerKit

final class QueryAnalysisTests: XCTestCase {
    func testEstimateMatchesSearchProbabilityIncludingAutoTrinket() async throws {
        let item = try XCTUnwrap(ItemCatalog.findById("runic_blade"))
        let requirement = try ItemRequirement(key: 1, item: item, upgrade: 3, kind: .weapon)
        for automatic in [false, true] {
            let query = try SearchRequest(requirements: [requirement], maximumDepth: 14,
                                          autoApplyTrinket: automatic)
            let analysis = try QueryAnalysis.analyze(QueryDocument.encode(query))
            XCTAssertFalse(analysis.impossible)
            XCTAssertGreaterThan(try XCTUnwrap(analysis.probability), 0)
            XCTAssertTrue(analysis.label.hasPrefix("Match probability ≈ 1 in "))
            let session = try await ProductionSeedFinderEngine().startSearch(query, workers: 1)
            let status = try await session.status()
            await session.cancel()
            await session.close()
            XCTAssertEqual(try XCTUnwrap(analysis.probability), status.matchProbability)
        }
    }

    func testEstimateRespondsToFloorLimitAndImpossibleQuery() throws {
        let requirement = try ItemRequirement(key: 1, item: nil, upgrade: 4, kind: .ring,
                                              upgradeMatch: .exactly)
        let shallow = try QueryAnalysis.analyze(QueryDocument.encode(
            SearchRequest(requirements: [requirement], maximumDepth: 14)))
        XCTAssertTrue(shallow.impossible)
        XCTAssertNil(shallow.probability)
        XCTAssertEqual(shallow.label, "Impossible query")

        let deep = try QueryAnalysis.analyze(QueryDocument.encode(
            SearchRequest(requirements: [requirement], maximumDepth: 24)))
        XCTAssertFalse(deep.impossible)
        XCTAssertGreaterThan(try XCTUnwrap(deep.probability), 0)
    }

    func testInvalidQueryDoesNotProduceAnEstimate() {
        XCTAssertThrowsError(try QueryAnalysis.analyze(Data("not json".utf8)))
        XCTAssertThrowsError(try QueryAnalysis.analyze(Data()))
    }

    func testCompactEstimateMatchesWebPresentation() {
        XCTAssertEqual(QueryAnalysis(impossible: false, probability: 1 / 5_090).label,
                       "Match probability ≈ 1 in 5.09 K")
        XCTAssertEqual(QueryAnalysis(impossible: false, probability: 1 / 5_000).label,
                       "Match probability ≈ 1 in 5 K")
        XCTAssertEqual(QueryAnalysis(impossible: false, probability: 1 / 1_250_000).label,
                       "Match probability ≈ 1 in 1.25 M")
    }
}
