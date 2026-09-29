import Foundation

public struct SavedQuery: Codable, Sendable {
    public var floorRequirements: [FloorRequirement]
    public var arcaneResin: Int
    public var arcaneResinAuto: Bool
    public var arcaneResinFilter: ArcaneResinFilter
    public var slotCount: Int { requirements.slotCount + floorRequirements.count + (arcaneResinAuto || arcaneResin > 0 ? 1 : 0) }
    public var requirements: [ItemRequirement]
    public var autoApplyTrinket: Bool
    public var maximumDepth: Int
    public var requireBlacksmith: Bool
    public var excludeBlacksmithRewards: Bool
    public var wandmakerQuest: WandmakerQuest?
    public var challenges: Int
    public init(requirements: [ItemRequirement] = [], maximumDepth: Int = 24,
                requireBlacksmith: Bool = false, excludeBlacksmithRewards: Bool = false,
                wandmakerQuest: WandmakerQuest? = nil,
                challenges: Int = 0, autoApplyTrinket: Bool = true,
                arcaneResin: Int = 0, arcaneResinFilter: ArcaneResinFilter = .init(), arcaneResinAuto: Bool = false, floorRequirements: [FloorRequirement] = []) {
        self.requirements = requirements; self.maximumDepth = maximumDepth
        self.requireBlacksmith = requireBlacksmith
        self.excludeBlacksmithRewards = excludeBlacksmithRewards
        self.wandmakerQuest = wandmakerQuest
        self.challenges = challenges
        self.autoApplyTrinket = autoApplyTrinket
        self.floorRequirements = floorRequirements
        self.arcaneResinAuto = arcaneResinAuto
        self.arcaneResin = arcaneResin; self.arcaneResinFilter = arcaneResinFilter
    }
    private enum CodingKeys: String, CodingKey {
        case requirements, maximumDepth, requireBlacksmith, excludeBlacksmithRewards
        case wandmakerQuest, challenges, autoApplyTrinket, arcaneResin, arcaneResinFilter, arcaneResinAuto, floorRequirements
    }
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        floorRequirements = try container.decodeIfPresent([FloorRequirement].self, forKey: .floorRequirements) ?? []
        arcaneResinAuto = try container.decodeIfPresent(Bool.self, forKey: .arcaneResinAuto) ?? false
        arcaneResin = try container.decodeIfPresent(Int.self, forKey: .arcaneResin) ?? 0
        arcaneResinFilter = try container.decodeIfPresent(ArcaneResinFilter.self, forKey: .arcaneResinFilter) ?? .init()
        autoApplyTrinket = try container.decodeIfPresent(Bool.self, forKey: .autoApplyTrinket) ?? false
        requirements = try container.decode([ItemRequirement].self, forKey: .requirements)
        // Queries saved before empty boss floors were removed may hold 5/10/15;
        // snap them to the equivalent limit below.
        maximumDepth = FloorLimits.normalize(try container.decode(Int.self, forKey: .maximumDepth))
        requireBlacksmith = try container.decode(Bool.self, forKey: .requireBlacksmith)
        excludeBlacksmithRewards = try container.decodeIfPresent(
            Bool.self, forKey: .excludeBlacksmithRewards) ?? false
        // A quest id a newer build knows falls back to "any" rather than
        // discarding the whole saved query.
        wandmakerQuest = (try? container.decodeIfPresent(WandmakerQuest.self, forKey: .wandmakerQuest)) ?? nil
        // Queries saved while the fast-mode toggle existed carry a `fastMode`
        // key; it has no coding key any more, so decoding skips it and the
        // query loads as an ordinary full search.
        challenges = try container.decodeIfPresent(Int.self, forKey: .challenges) ?? 0
    }
    public func validated() -> SavedQuery? {
        guard floorRequirements.allSatisfy(\.isValid), Set(floorRequirements.map(\.depth)).count == floorRequirements.count else { return nil }
        guard (0...65535).contains(arcaneResin), arcaneResinFilter.isValid else { return nil }
        guard (1...SearchLimits.maxDepth).contains(maximumDepth), (0...SearchLimits.challengeMask).contains(challenges) else { return nil }
        for requirement in requirements {
            if let item = requirement.item, ItemCatalog.findById(item.id) != item { return nil }
            // The validating initializer also checks every effect name
            // against the catalog.
            guard (try? ItemRequirement(key: requirement.key, item: requirement.item,
                upgrade: requirement.upgrade, effect: requirement.effect, kind: requirement.kind,
                tier: requirement.tier, tierMatch: requirement.tierMatch,
                upgradeMatch: requirement.upgradeMatch, source: requirement.source,
                identityGroup: requirement.identityGroup,
                maximumDepth: requirement.maximumDepth,
                requireUncursed: requirement.requireUncursed,
                alternativeGroup: requirement.alternativeGroup,
                levelSum: requirement.levelSum, selectTrinket: requirement.selectTrinket, trinketTransmutations: requirement.trinketTransmutations, artifactTransmutations: requirement.artifactTransmutations, blanket: requirement.blanket)) != nil else { return nil }
        }
        // A combined-level group that no longer adds up, or a same-item group
        // constrained twice, still loads — the editor shows why the search
        // cannot start — but the engine would refuse it, so the group checks
        // are the request's, not this loader's.
        return self
    }
}

public struct QueryPreset: Codable, Hashable, Identifiable, Sendable {
    public let id: UUID
    public var name: String
    public var query: SavedQuery

    public init(id: UUID = UUID(), name: String, query: SavedQuery) {
        self.id = id; self.name = name; self.query = query
    }
}

extension SavedQuery: Hashable {}

public enum BuiltInPresets {
    // Every preset keeps `SavedQuery`'s default of applying the chosen
    // trinket automatically. The ids are fresh rather than the retired
    // presets' (…272101 to …272105), so an id a user hid or stored never
    // lands on a different query.
    public static let all: [QueryPreset] = [disintegrate, guerillaAssassin, ringOfWealth, necromancer, bloodBerserker]

    public static let disintegrate = QueryPreset(
        id: UUID(uuidString: "C3DB688D-3D7D-43F0-B10E-9BCBEA272106")!,
        name: "DISINTEGRATE",
        query: SavedQuery(requirements: [
            try! ItemRequirement(key: 1, item: ItemCatalog.findById("wand_disintegration"), upgrade: 3,
                                 kind: .wand, upgradeMatch: .atLeast),
            try! ItemRequirement(key: 2, item: ItemCatalog.findById("wand_disintegration"), upgrade: 0,
                                 kind: .wand, upgradeMatch: .any),
            try! ItemRequirement(key: 3, item: ItemCatalog.findById("wand_disintegration"), upgrade: 0,
                                 kind: .wand, upgradeMatch: .any),
            try! ItemRequirement(key: 4, item: ItemCatalog.findById("eye_of_newt"), upgrade: 0,
                                 kind: .trinket, upgradeMatch: .any, trinketTransmutations: 1),
            try! ItemRequirement(key: 5, item: ItemCatalog.findById("ring_energy"), upgrade: 2,
                                 kind: .ring, upgradeMatch: .atLeast),
        ], maximumDepth: 19))

    public static let guerillaAssassin = QueryPreset(
        id: UUID(uuidString: "C3DB688D-3D7D-43F0-B10E-9BCBEA272107")!,
        name: "Guerilla Assassin",
        query: SavedQuery(requirements: [
            try! ItemRequirement(key: 1, item: ItemCatalog.findById("assassins_blade"), upgrade: 3,
                                 modifier: "Blooming", kind: .weapon, upgradeMatch: .exactly,
                                 maximumDepth: 7),
            try! ItemRequirement(key: 2, item: nil, upgrade: 0, modifier: "Camouflage",
                                 kind: .armor, upgradeMatch: .any),
            try! ItemRequirement(key: 3, item: ItemCatalog.findById("ring_arcana"), upgrade: 2,
                                 kind: .ring, upgradeMatch: .atLeast),
        ]))

    /// Floor 17 must be a farming floor: dark, with a garden or secret garden.
    public static let ringOfWealth = QueryPreset(
        id: UUID(uuidString: "C3DB688D-3D7D-43F0-B10E-9BCBEA272108")!,
        name: "Ring of Wealth",
        query: SavedQuery(requirements: [
            try! ItemRequirement(key: 1, item: ItemCatalog.findById("ring_wealth"), upgrade: 4,
                                 kind: .ring, upgradeMatch: .exactly),
            try! ItemRequirement(key: 2, item: ItemCatalog.findById("dried_rose"), upgrade: 0,
                                 kind: .artifact, upgradeMatch: .any, maximumDepth: 9),
            try! ItemRequirement(key: 3, item: nil, upgrade: 3, kind: .armor,
                                 tier: 4, tierMatch: .atMost, upgradeMatch: .exactly,
                                 maximumDepth: 4),
            try! ItemRequirement(key: 4, item: nil, upgrade: 3, kind: .weapon,
                                 tier: 4, tierMatch: .atMost, upgradeMatch: .exactly,
                                 maximumDepth: 9),
            try! ItemRequirement(key: 5, item: ItemCatalog.findById("dimensional_sundial"), upgrade: 0,
                                 kind: .trinket, upgradeMatch: .any, trinketTransmutations: 1),
        ], floorRequirements: [
            FloorRequirement(depth: 17, feeling: "dark", anyRooms: ["garden", "secret_garden"]),
        ]))

    public static let necromancer = QueryPreset(
        id: UUID(uuidString: "C3DB688D-3D7D-43F0-B10E-9BCBEA272109")!,
        name: "Necromancer",
        query: SavedQuery(requirements: [
            try! ItemRequirement(key: 1, item: ItemCatalog.findById("wand_corruption"), upgrade: 3,
                                 kind: .wand, upgradeMatch: .exactly),
            try! ItemRequirement(key: 2, item: nil, upgrade: 3, kind: .weapon,
                                 tier: 5, tierMatch: .exactly, upgradeMatch: .exactly),
            try! ItemRequirement(key: 3, item: ItemCatalog.findById("plate_armor"), upgrade: 3,
                                 kind: .armor, upgradeMatch: .exactly),
        ], maximumDepth: 14, wandmakerQuest: .corpseDust))

    public static let bloodBerserker = QueryPreset(
        id: UUID(uuidString: "C3DB688D-3D7D-43F0-B10E-9BCBEA272110")!,
        name: "Blood Berserker",
        query: SavedQuery(requirements: [
            try! ItemRequirement(key: 1, item: nil, upgrade: 3, modifier: "Vampiric", kind: .weapon,
                                 tier: 5, tierMatch: .exactly, upgradeMatch: .exactly),
            try! ItemRequirement(key: 2, item: ItemCatalog.findById("plate_armor"), upgrade: 3,
                                 modifier: "Thorns", kind: .armor, upgradeMatch: .exactly),
            try! ItemRequirement(key: 3, item: ItemCatalog.findById("ring_arcana"), upgrade: 4,
                                 kind: .ring, upgradeMatch: .exactly),
            try! ItemRequirement(key: 4, item: ItemCatalog.findById("chalice_of_blood"), upgrade: 0,
                                 kind: .artifact, upgradeMatch: .any),
        ]))
}

extension Array where Element == ItemRequirement {
    /// The list with its rows keyed 1…n in order. A key only names a row
    /// within one list, so a loaded or imported list starts from these:
    /// small keys every platform's integers hold exactly, none of them zero
    /// and none repeated.
    public func withKeysInOrder() -> [ItemRequirement] {
        var keyed = self
        for index in keyed.indices { keyed[index].key = Int64(index + 1) }
        return keyed
    }
}

extension SavedQuery {
    /// The query with its requirements keyed 1…n in order.
    public func withKeysInOrder() -> SavedQuery {
        var keyed = self
        keyed.requirements = requirements.withKeysInOrder()
        return keyed
    }
}

public enum QueryPersistence {
    public static func encode(_ query: SavedQuery) -> String? {
        guard let data = try? JSONEncoder().encode(query) else { return nil }
        return String(data: data, encoding: .utf8)
    }
    /// The saved query, its requirements keyed 1…n in order: a key only
    /// names a row within one list, and earlier builds saved random 64-bit
    /// keys (macOS) and key 0 for new rows (iOS).
    public static func decode(_ text: String) -> SavedQuery {
        guard let data = text.data(using: .utf8), let value = try? JSONDecoder().decode(SavedQuery.self, from: data),
              let validated = value.validated() else { return SavedQuery() }
        return validated.withKeysInOrder()
    }
}

/// The worker count — how many search threads the engine spawns — as a
/// device-local preference.
///
/// Deliberately not a `SavedQuery` field: it describes this machine's cores,
/// not the seeds a query matches, so it stays out of query documents, presets,
/// results exports and share links. A query that
/// travels to another machine must search the same seeds there.
///
/// The stored form is a plain `UserDefaults` integer under
/// `WorkerPersistence.defaultsKey`, read through `@AppStorage` like the other
/// preferences; `unset` is the absent-value default, so a machine that has
/// never touched the selector searches on every core.
public enum WorkerPersistence {
    public static let defaultsKey = "workerCount"

    /// The stored value meaning "never chosen": use every available core.
    /// It is also what the FFI reads as "all cores", so it needs no
    /// translation on the way down.
    public static let unset = 0

    /// The saved preference read back against this machine's ceiling: unset
    /// (or nonsense, including a negative left by a hand-edited defaults
    /// entry) means every core, and a count saved on a bigger machine is
    /// clamped down rather than discarded.
    public static func resolve(saved: Int, ceiling: Int) -> Int {
        let ceiling = max(1, ceiling)
        guard saved > 0 else { return ceiling }
        return clamp(saved, ceiling: ceiling)
    }

    /// A chosen count confined to `1...ceiling`.
    public static func clamp(_ value: Int, ceiling: Int) -> Int {
        min(max(1, value), max(1, ceiling))
    }
}

public enum PresetPersistence {
    public static func encode(_ presets: [QueryPreset]) -> String? {
        guard let data = try? JSONEncoder().encode(presets) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    public static func decode(_ text: String) -> [QueryPreset] {
        // Decode per element so one unreadable preset (for example, written
        // by a newer build with kinds this build predates) drops only
        // itself, never the whole collection.
        guard let data = text.data(using: .utf8),
              let elements = (try? JSONSerialization.jsonObject(with: data)) as? [Any] else { return [] }
        let presets = elements.compactMap { element -> QueryPreset? in
            guard JSONSerialization.isValidJSONObject(element),
                  let elementData = try? JSONSerialization.data(withJSONObject: element) else { return nil }
            return try? JSONDecoder().decode(QueryPreset.self, from: elementData)
        }
        return presets
            .filter { !$0.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && $0.query.validated() != nil }
            .map { preset in
                // Keyed in order, as a saved query loads.
                var keyed = preset
                keyed.query = preset.query.withKeysInOrder()
                return keyed
            }
    }
}

public extension SavedQuery {
    func searchRequest() throws -> SearchRequest {
        try SearchRequest(requirements: requirements, maximumDepth: maximumDepth,
                          requireBlacksmith: requireBlacksmith, excludeBlacksmithRewards: excludeBlacksmithRewards,
                          wandmakerQuest: wandmakerQuest, challenges: challenges, autoApplyTrinket: autoApplyTrinket, arcaneResin: arcaneResin, arcaneResinFilter: arcaneResinFilter, arcaneResinAuto: arcaneResinAuto, floorRequirements: floorRequirements)
    }
}
