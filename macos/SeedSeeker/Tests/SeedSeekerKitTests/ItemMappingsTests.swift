import Foundation
import XCTest
@testable import SeedSeekerKit

final class ItemMappingsTests: XCTestCase {
    func testNativeScoutReturnsAllMappingsIndependentOfChallengesAndTrinkets() async throws {
        let artwork = ItemMappingArtwork.shared
        XCTAssertEqual(artwork.categories.keys.sorted(), ["potions", "rings", "scrolls"])
        XCTAssertTrue(artwork.categories.values.allSatisfy { $0.iconSizes.count == 12 })
        let engine = ProductionSeedFinderEngine()
        let world = try await engine.scoutSeed("ABC-DEF-GHI", challenges: 0)
        let mappings = try XCTUnwrap(world.itemMappings)
        XCTAssertEqual(mappings.scrolls[0], ScoutItemMapping(name: "Scroll of upgrade", appearance: "TIWAZ", spriteIndex: 315))
        for group in [mappings.scrolls, mappings.potions, mappings.rings] {
            XCTAssertEqual(group.count, 12)
            XCTAssertEqual(Set(group.map(\.spriteIndex)).count, 12)
            XCTAssertEqual(Set(group.map(\.appearance)).count, 12)
        }
        XCTAssertEqual(mappings.rings.map { $0.spriteIndex - 224 }, world.ringGems.ordinals)
        let changed = try await engine.scoutSeed(world.seed, challenges: 1, query: nil, trinket: "none")
        XCTAssertEqual(mappings, changed.itemMappings)
    }

    private func packet(version: String = "SSC7", mutation: String = "") -> Data {
        var data = Data()
        func bytes<S: Sequence>(_ value: S) where S.Element == UInt8 { data.append(contentsOf: value) }
        func u8(_ value: Int) { data.append(UInt8(value)) }
        func u16(_ value: Int) { bytes([UInt8(value >> 8), UInt8(value & 255)]) }
        bytes(version.utf8); u8(11); bytes("AAA-AAA-AAA".utf8)
        bytes((0..<12).map(UInt8.init)); u8(0); u16(0)
        if version == "SSC3" { return data }
        u8(17)
        for item in ItemCatalog.trinkets { u16(item.id.utf8.count); bytes(item.id.utf8) }
        if version == "SSC4" { return data }
        u8(0)
        if version == "SSC5" { return data }
        u16(0)
        if version == "SSC6" { return data }
        for base in [304, 352, 224] {
            for index in 0..<12 {
                let name = mutation == "blank" ? "" : "Item \(index)"
                let appearance = mutation == "duplicate" ? "Same" : "Appearance \(index)"
                u16(name.utf8.count); bytes(name.utf8)
                u16(appearance.utf8.count); bytes(appearance.utf8)
                u16(mutation == "range" ? 65535 : base + (mutation == "ring" && base == 224 ? (index + 1) % 12 : index))
            }
        }
        return data
    }

    func testLegacyPacketsOmitMappingsAndMalformedExtensionsFail() throws {
        for version in ["SSC3", "SSC4", "SSC5", "SSC6"] {
            XCTAssertNil(try ScoutCodec.decode(packet(version: version)).itemMappings)
        }
        let valid = packet()
        XCTAssertNotNil(try ScoutCodec.decode(valid).itemMappings)
        XCTAssertThrowsError(try ScoutCodec.decode(valid.dropLast()))
        XCTAssertThrowsError(try ScoutCodec.decode(valid + Data([0])))
        for mutation in ["blank", "duplicate", "range", "ring"] {
            XCTAssertThrowsError(try ScoutCodec.decode(packet(mutation: mutation)))
        }
    }
}
