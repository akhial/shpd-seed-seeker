// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation

public struct ScoutItemMapping: Equatable, Sendable {
    public let name: String
    public let appearance: String
    public let spriteIndex: Int
    public var label: String { "\(appearance) — \(name)" }
    public init(name: String, appearance: String, spriteIndex: Int) {
        self.name = name; self.appearance = appearance; self.spriteIndex = spriteIndex
    }
}

public struct ScoutItemMappings: Equatable, Sendable {
    public let scrolls: [ScoutItemMapping]
    public let potions: [ScoutItemMapping]
    public let rings: [ScoutItemMapping]
}

public struct ItemMappingArtwork: Decodable, Sendable {
    public struct Category: Decodable, Sendable {
        public let spriteSize: [Int]
        public let iconBase: Int
        public let iconSizes: [[Int]]
    }
    public let slotSize: Int
    public let slotGap: Int
    public let categories: [String: Category]

    public static let shared: ItemMappingArtwork = {
        let installed = Bundle.main.url(forResource: "item-mapping-art", withExtension: "json")
        let checkout = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("android/app/src/main/assets/third_party/shattered-pixel-dungeon/item-mapping-art.json")
        guard let data = try? Data(contentsOf: installed ?? checkout),
              let artwork = try? JSONDecoder().decode(Self.self, from: data) else {
            preconditionFailure("the bundled journal artwork metadata is missing or unreadable")
        }
        return artwork
    }()
}
