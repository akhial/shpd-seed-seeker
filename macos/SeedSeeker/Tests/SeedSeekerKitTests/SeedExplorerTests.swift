import Foundation
@testable import SeedSeekerKit
import XCTest

final class SeedListCodecTests: XCTestCase {
    func testDecodeNormalizesLineEndingsCaseBOMBlanksAndDuplicates() throws {
        let input = Data("\u{feff}aaa-aaa-aaa\r\n\r\nBBB-BBB-BBB\nAAA-AAA-AAA\n".utf8)

        XCTAssertEqual(try SeedListCodec.decode(input), ["AAA-AAA-AAA", "BBB-BBB-BBB"])
        XCTAssertEqual(String(data: try SeedListCodec.encode(["aaa-aaa-aaa", "BBB-BBB-BBB"]),
                              encoding: .utf8),
                       "AAA-AAA-AAA\nBBB-BBB-BBB\n")
    }

    func testDecodeReportsTheInvalidNonblankLine() {
        let input = Data("AAA-AAA-AAA\r\n\r\nnot-a-seed\r\n".utf8)

        XCTAssertThrowsError(try SeedListCodec.decode(input)) { error in
            XCTAssertEqual(error as? SeedListCodecError,
                           .invalidSeed(line: 3, value: "not-a-seed"))
        }
        XCTAssertThrowsError(try SeedListCodec.decode(Data([0xff]))) { error in
            XCTAssertEqual(error as? SeedListCodecError, .invalidUTF8)
        }
        XCTAssertThrowsError(try SeedListCodec.decode(Data(" AAA-AAA-AAA \n".utf8))) { error in
            XCTAssertEqual(error as? SeedListCodecError,
                           .invalidSeed(line: 1, value: " AAA-AAA-AAA "))
        }
        XCTAssertThrowsError(try SeedListCodec.decode(Data("ſſſ-ſſſ-ſſſ\n".utf8)))
        XCTAssertThrowsError(try SeedListCodec.encode(["ııı-ııı-ııı"]))
    }

    func testSeedListLimitAppliesAfterDeduplication() throws {
        let maximum = (0..<SeedListCodec.maximumSeedCount).map(seedCode)
        XCTAssertEqual(try SeedListCodec.decode(SeedListCodec.encode(maximum)).count,
                       SeedListCodec.maximumSeedCount)
        XCTAssertNoThrow(try SeedListCodec.encode(maximum + [maximum[0]]))
        XCTAssertThrowsError(try SeedListCodec.encode(maximum + [seedCode(maximum.count)])) { error in
            XCTAssertEqual(error as? SeedListCodecError,
                           .tooManySeeds(maximum: SeedListCodec.maximumSeedCount))
        }
    }

    func testFilterCodecWrapsTheExistingQueryAndFixedWidthSeeds() throws {
        let plate = try XCTUnwrap(ItemCatalog.findById("plate_armor"))
        let requirement = try ItemRequirement(key: 1, item: plate, upgrade: 2,
                                              kind: .armor, upgradeMatch: .exactly)
        let request = try SearchRequest(requirements: [requirement], maximumDepth: 12,
                                        challenges: 32)
        let query = try QueryCodec.encode(request)
        let packet = try FilterCodec.encode(request: request,
                                            seeds: ["AAA-AAA-AAA", "BBB-BBB-BBB"])

        XCTAssertEqual(Data(packet.prefix(4)), Data("SFF1".utf8))
        XCTAssertEqual(Int(packet[4]) << 8 | Int(packet[5]), query.count)
        XCTAssertEqual(packet.subdata(in: 6..<(6 + query.count)), query)
        let countOffset = 6 + query.count
        XCTAssertEqual(Array(packet[countOffset..<(countOffset + 2)]), [0, 2])
        XCTAssertEqual(String(data: packet.suffix(22), encoding: .ascii),
                       "AAA-AAA-AAABBB-BBB-BBB")
        XCTAssertThrowsError(try FilterCodec.encode(request: request, seeds: []))
        XCTAssertThrowsError(try FilterCodec.encode(request: request, seeds: ["bad"]))
    }

    func testProductionFilterFFIRoundTrip() async throws {
        let engine = ProductionSeedFinderEngine()
        let seed = "AAA-AAA-AAA"
        let world = try await engine.scoutSeed(seed, challenges: 0)
        let candidate = try XCTUnwrap(world.items.first { $0.upgrade > 0 })
        let requirement = try ItemRequirement(key: 1, item: candidate.item,
                                              upgrade: candidate.upgrade,
                                              kind: candidate.item.kind,
                                              upgradeMatch: .exactly)

        let results = try await engine.filterSeeds([seed, seed], matching:
            SearchRequest(requirements: [requirement]))

        XCTAssertEqual(results, [
            SeedResult(seed: seed, matchedRequirements: 1),
            SeedResult(seed: seed, matchedRequirements: 1),
        ])
    }

    private func seedCode(_ value: Int) -> String {
        var value = value
        var letters = [Character](repeating: "A", count: 9)
        for index in letters.indices.reversed() {
            letters[index] = Character(UnicodeScalar(65 + value % 26)!)
            value /= 26
        }
        let text = String(letters)
        return "\(text.prefix(3))-\(text.dropFirst(3).prefix(3))-\(text.suffix(3))"
    }
}

@MainActor
final class SeedExplorerControllerTests: XCTestCase {
    func testTextSearchIgnoresCaseDashesAndWhitespace() throws {
        let controller = SearchController(engine: FakeFilterEngine(matches: []))
        try controller.replaceWithImportedSeeds(["ABC-DEF-GHI", "XYZ-XYZ-XYZ"])

        for query in ["abcdef", "c-de", " G H I "] {
            controller.seedSearchText = query
            XCTAssertEqual(controller.results.map(\.seed), ["ABC-DEF-GHI"])
        }
    }

    func testTextAndSemanticFiltersNeverDestroyTheImportedBaseList() async throws {
        let seeds = ["AAA-AAA-AAA", "BBB-BBB-BBB", "CCC-CCC-CCC"]
        let controller = SearchController(engine: FakeFilterEngine(
            matches: ["AAA-AAA-AAA", "CCC-CCC-CCC"]))
        try controller.replaceWithImportedSeeds(seeds)

        XCTAssertTrue(controller.isImportedList)
        XCTAssertEqual(controller.baseResults.map(\.seed), seeds)
        controller.seedSearchText = "bbb"
        XCTAssertEqual(controller.results.map(\.seed), ["BBB-BBB-BBB"])

        controller.seedSearchText = ""
        let plate = try XCTUnwrap(ItemCatalog.findById("plate_armor"))
        let requirement = try ItemRequirement(key: 1, item: plate, upgrade: 2,
                                              kind: .armor, upgradeMatch: .exactly)
        controller.filterSeeds(matching: try SearchRequest(requirements: [requirement]))
        while controller.isFiltering { await Task.yield() }

        XCTAssertEqual(controller.baseResults.map(\.seed), seeds)
        XCTAssertEqual(controller.results.map(\.seed), ["AAA-AAA-AAA", "CCC-CCC-CCC"])
        controller.seedSearchText = "ccc"
        XCTAssertEqual(controller.results.map(\.seed), ["CCC-CCC-CCC"])

        controller.clearFilter()
        XCTAssertFalse(controller.hasActiveFilter)
        XCTAssertEqual(controller.results.map(\.seed), seeds)
    }
}

private struct FakeFilterEngine: SeedFinderEngine {
    let matches: Set<String>

    func startSearch(_ request: SearchRequest) async throws -> any SeedFinderSearchSession {
        throw FakeEngineError.unused
    }

    func filterSeeds(_ seeds: [String], matching request: SearchRequest) async throws -> [SeedResult] {
        seeds.reversed().filter(matches.contains).map {
            SeedResult(seed: $0, matchedRequirements: request.requirements.count)
        }
    }

    func scoutSeed(_ seed: String, challenges: Int) async throws -> ScoutWorld {
        throw FakeEngineError.unused
    }
}

private enum FakeEngineError: Error { case unused }
