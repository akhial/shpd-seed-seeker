import Foundation
import XCTest
@testable import SeedSeekerKit

final class FloorRequirementsTests: XCTestCase {
    func testScoutFarmingLabelsUseGeneratedRoomsAndFeelings() async throws {
        let actual = try await ProductionSeedFinderEngine().scoutSeed("DJG-HMA-ULY", challenges: 0)
        XCTAssertEqual(actual.floorRooms.count, 20)
        XCTAssertTrue(actual.isFarmingFloor(17))
        let cases: [(Int, FloorFeeling, Set<String>, Bool)] = [
            (7, .dark, ["garden"], true), (17, .dark, ["secret_garden"], true),
            (22, .dark, ["garden", "secret_garden"], true), (8, .dark, ["garden"], false),
            (7, .grass, ["garden"], false), (17, .dark, [], false), (22, .dark, ["quest_rot_garden"], false),
        ]
        for (depth, feeling, rooms, expected) in cases {
            let world = ScoutWorld(seed: actual.seed, items: [], feelings: [depth: feeling], floorRooms: [depth: rooms])
            XCTAssertEqual(world.isFarmingFloor(depth), expected)
            XCTAssertFalse(ScoutWorld(seed: actual.seed, items: [], feelings: [depth: feeling]).isFarmingFloor(depth))
        }
    }

    func testIndependentFarmingFloorsRaiseScopeAndValidate() throws {
        var query = SavedQuery(maximumDepth: 4)
        for depth in [22, 7, 17] { query.toggleFarmingFloor(depth) }
        XCTAssertEqual(query.maximumDepth, 22)
        XCTAssertEqual(query.floorRequirements.map(\.depth), [7, 17, 22])
        XCTAssertTrue(query.floorRequirements.allSatisfy(\.isFarming))
        XCTAssertEqual(try query.searchRequest().slotCount, 3)
        query.toggleFarmingFloor(17)
        XCTAssertEqual(query.floorRequirements.map(\.depth), [7, 22])
        XCTAssertEqual(query.maximumDepth, 22)
        query.maximumDepth = 16
        XCTAssertThrowsError(try query.searchRequest())
    }

    func testFloorQueriesSurviveEveryFormat() throws {
        var query = SavedQuery(autoApplyTrinket: false)
        for depth in FloorRequirement.farmingFloors { query.toggleFarmingFloor(depth) }
        query.floorRequirements.append(.init(depth: 9, feeling: "secrets", rooms: ["secret_library"], anyRooms: ["garden", "secret_garden"]))
        let request = try query.searchRequest()
        let persisted = try JSONDecoder().decode(SavedQuery.self, from: JSONEncoder().encode(query))
        let requestCopy = try JSONDecoder().decode(SearchRequest.self, from: JSONEncoder().encode(request))
        XCTAssertEqual(try QueryDocument.encode(requestCopy), try QueryDocument.encode(request))
        let linked = try DeepLink.decode(DeepLink.encodeLink(for: query))
        let decoded = try ResultsExport.decodeQuery(ResultsExport.encodeQuery(query))
        let exported = try ResultsExport.decode(ResultsExport.encode(query, seeds: ["AAA-AAA-AAA"], appVersion: "test"))
        for restored in [persisted, linked, decoded, exported.query] { XCTAssertEqual(restored, query) }
        XCTAssertTrue(try ResultsExport.decodeQuery(["requirements": []]).floorRequirements.isEmpty)
        XCTAssertNotNil(try QueryAnalysis.analyze(QueryDocument.encode(request)).probability)
    }

    func testNativeFilteringAndScoutEnforceFloorOnlyConditions() async throws {
        let engine = ProductionSeedFinderEngine()
        let world = try await engine.scoutSeed("AAA-AAA-AAA", challenges: 0)
        let feeling = try XCTUnwrap(world.feelings[7])
        let names = ["none", "chasm", "water", "grass", "dark", "large", "traps", "secrets"]
        var query = try SearchRequest(requirements: [], floorRequirements: [.init(depth: 7, feeling: names[feeling.rawValue])])
        let kept = try await engine.filterSeeds(query, seeds: [world.seed])
        XCTAssertEqual(kept, [world.seed])
        XCTAssertEqual(try ScoutMatches.mark(seed: world.seed, challenges: 0, query: query).matchedRequirements, 1)
        query.floorRequirements = [.init(depth: 7, feeling: feeling == .dark ? "water" : "dark")]
        let rejected = try await engine.filterSeeds(query, seeds: [world.seed])
        XCTAssertTrue(rejected.isEmpty)
        XCTAssertEqual(try ScoutMatches.mark(seed: world.seed, challenges: 0, query: query).matchedRequirements, 0)
    }
}
