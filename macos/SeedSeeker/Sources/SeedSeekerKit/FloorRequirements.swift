// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation

public struct FloorRequirement: Codable, Hashable, Sendable {
    public static let farmingFloors = [7, 17, 22]
    public var depth: Int
    public var feeling: String?
    public var rooms: [String]
    public var anyRooms: [String]

    public init(depth: Int, feeling: String? = nil, rooms: [String] = [], anyRooms: [String] = []) {
        self.depth = depth; self.feeling = feeling; self.rooms = rooms; self.anyRooms = anyRooms
    }
    private enum CodingKeys: String, CodingKey { case depth, feeling, rooms; case anyRooms = "any_rooms" }
    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        depth = try values.decode(Int.self, forKey: .depth)
        feeling = try values.decodeIfPresent(String.self, forKey: .feeling)
        rooms = try values.decodeIfPresent([String].self, forKey: .rooms) ?? []
        anyRooms = try values.decodeIfPresent([String].self, forKey: .anyRooms) ?? []
    }
    public var isValid: Bool {
        (1...24).contains(depth) && depth % 5 != 0 &&
        (feeling == nil || ["none", "chasm", "water", "grass", "dark", "large", "traps", "secrets"].contains(feeling!)) &&
        (feeling != nil || !rooms.isEmpty || !anyRooms.isEmpty)
    }
    public var isFarming: Bool {
        Self.farmingFloors.contains(depth) && feeling == "dark" && rooms.isEmpty &&
        anyRooms.count == 2 && Set(anyRooms) == Set(["garden", "secret_garden"])
    }
    public var summary: String {
        (["Floor \(depth)"] + [feeling].compactMap { $0 } + rooms +
         (anyRooms.isEmpty ? [] : [anyRooms.joined(separator: " / ")]))
            .joined(separator: " · ").replacingOccurrences(of: "_", with: " ")
    }
    var document: [String: Any] {
        var value: [String: Any] = ["depth": depth]
        if let feeling { value["feeling"] = feeling }
        if !rooms.isEmpty { value["rooms"] = rooms }
        if !anyRooms.isEmpty { value["any_rooms"] = anyRooms }
        return value
    }
}

public extension SavedQuery {
    mutating func toggleFarmingFloor(_ depth: Int) {
        precondition(FloorRequirement.farmingFloors.contains(depth))
        let selected = floorRequirements.contains { $0.depth == depth && $0.isFarming }
        floorRequirements.removeAll { $0.depth == depth }
        if !selected {
            floorRequirements.append(.init(depth: depth, feeling: "dark", anyRooms: ["garden", "secret_garden"]))
            maximumDepth = max(maximumDepth, depth)
        }
        floorRequirements.sort { $0.depth < $1.depth }
    }
}
