// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import SeedSeekerKit

/// User-facing wording stays with the Android app, even where the shared
/// desktop models use a different diagnostic for the same invalid query.
enum AndroidCopy {
    /// Mirrors floorValidationProblem and validationProblem in SearchModels.kt.
    /// Preserve their ordering: a misplaced floor is explained before the
    /// item constraints, and stack errors retain their actual counts/totals.
    static func validationMessage(for query: SavedQuery) -> String? {
        let requirements = query.requirements
        let floors = query.floorRequirements
        if Set(floors.map(\.depth)).count != floors.count {
            return "Each floor can have only one requirement."
        }
        if let floor = floors.first(where: { $0.depth > query.maximumDepth }) {
            return "Floor \(floor.depth) exceeds the floor limit of \(query.maximumDepth)."
        }
        if !(0...65_535).contains(query.arcaneResin) {
            return "Arcane Resin must be 0..65535."
        }
        if requirements.isEmpty && query.arcaneResin == 0 && !query.arcaneResinAuto && floors.isEmpty {
            return "Add at least one requirement."
        }
        if !requirements.isEmpty && requirements.allSatisfy(\.blanket) {
            return "Add at least one ordinary requirement."
        }
        if requirements.slots.contains(where: { slot in
            guard let first = slot.first else { return false }
            return slot.contains { $0.blanket != first.blanket }
        }) {
            return "An either/or group cannot mix ordinary and blanket requirements."
        }

        let stacks = Dictionary(grouping: requirements.filter { $0.identityGroup != nil },
                                by: { $0.identityGroup! })
        for (_, members) in stacks.sorted(by: { $0.key < $1.key }) {
            if Set(members.map(\.kind.family)).count > 1 {
                return "The copies of a stack must share its category."
            }
            let units = Set(members.filter { !$0.isBare }.map { member in
                member.alternativeGroup.map { "alt:\($0)" } ?? "req:\(member.key)"
            })
            if units.count > 1 {
                return "Only one item of a stack can carry constraints; the extra copies are plain."
            }
        }

        let sums = Dictionary(grouping: requirements.filter { $0.levelSum != nil },
                              by: { $0.levelSum!.group })
        for (_, members) in sums.sorted(by: { $0.key < $1.key }) {
            if members.contains(where: { $0.kind.family != .ring }) {
                return "Only rings can count levels together."
            }
            let totals = Set(members.compactMap { $0.levelSum?.atLeast }).sorted()
            if totals.count > 1 {
                return "A stack must share one combined level (it has \(totals.map(String.init).joined(separator: " and ")))."
            }
            let reachable = min(members.reduce(0) { $0 + $1.maximumLevel },
                                SearchLimits.ringStackCapacity(members.count))
            if let needed = totals.first, needed > reachable {
                return "A combined level of \(needed) needs more items: these \(members.count) can reach \(reachable)."
            }
        }

        if !(1...SearchLimits.maxDepth).contains(query.maximumDepth) {
            return "Maximum floor must be 1..\(SearchLimits.maxDepth)"
        }
        if !(0...SearchLimits.challengeMask).contains(query.challenges) {
            return "Challenge mask must be 0..\(SearchLimits.challengeMask)"
        }
        if !query.arcaneResinFilter.isValid {
            return "Arcane Resin floor must be 1..\(SearchLimits.maxDepth)"
        }
        if floors.contains(where: { !(1...SearchLimits.maxDepth).contains($0.depth) || $0.depth % 5 == 0 }) {
            return "Choose a regular floor from 1 through 24."
        }
        if floors.contains(where: { !$0.isValid }) {
            return "Choose a feeling or room for each floor."
        }
        if requirements.contains(where: { $0.excludeResin && ($0.kind != .wand || $0.blanket) }) {
            return "Only an ordinary wand can exclude Auto resin"
        }
        if requirements.contains(where: { $0.blanket && ($0.identityGroup != nil || $0.levelSum != nil || $0.selectTrinket) }) {
            return "A blanket cannot request extra copies, combined levels, or trinket selection"
        }
        return nil
    }

    /// The C bridge returns an error category, not Android JNI's detailed
    /// codec diagnostic. Use Android's existing transfer fallback rather than
    /// exposing the desktop wrapper's different text or inventing a cause.
    static func importError(_ error: Error, source: String) -> String {
        if error is CocoaError || (error as? ResultsExportError)?.message == "Could not read the selected file." {
            return "Could not read the selected file."
        }
        return "The results could not be imported from \(source)."
    }

    static func linkError(_ error: Error) -> String {
        "This shared search link could not be read."
    }

    static func shareError(_ error: Error) -> String {
        if let error = error as? ModelValidationError, error == .emptyRequirements {
            return "Add at least one requirement to share a search."
        }
        return "This search could not be shared."
    }
}
