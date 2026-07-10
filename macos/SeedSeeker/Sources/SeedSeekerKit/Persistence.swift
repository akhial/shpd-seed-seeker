import Foundation

public struct SavedQuery: Codable, Sendable {
    public var requirements: [ItemRequirement]
    public var maximumDepth: Int
    public var requireBlacksmith: Bool
    public init(requirements: [ItemRequirement] = [], maximumDepth: Int = 24, requireBlacksmith: Bool = false) {
        self.requirements = requirements; self.maximumDepth = maximumDepth; self.requireBlacksmith = requireBlacksmith
    }
    public func validated() -> SavedQuery? {
        guard (1...24).contains(maximumDepth) else { return nil }
        for requirement in requirements {
            if let item = requirement.item, ItemCatalog.findById(item.id) != item { return nil }
            if let modifier = requirement.modifier,
               !ItemCatalog.modifiersFor(requirement.kind).contains(modifier) { return nil }
            guard (try? ItemRequirement(key: requirement.key, item: requirement.item,
                upgrade: requirement.upgrade, modifier: requirement.modifier, kind: requirement.kind,
                upgradeMatch: requirement.upgradeMatch, source: requirement.source,
                identityGroup: requirement.identityGroup)) != nil else { return nil }
        }
        return self
    }
}

public enum QueryPersistence {
    public static func encode(_ query: SavedQuery) -> String? {
        guard let data = try? JSONEncoder().encode(query) else { return nil }
        return String(data: data, encoding: .utf8)
    }
    public static func decode(_ text: String) -> SavedQuery {
        guard let data = text.data(using: .utf8), let value = try? JSONDecoder().decode(SavedQuery.self, from: data),
              let validated = value.validated() else { return SavedQuery() }
        return validated
    }
}
