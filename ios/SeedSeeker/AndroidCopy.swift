// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import SeedSeekerKit

/// User-facing wording stays with the Android app, even where the shared
/// desktop models use a different diagnostic for the same invalid query.
enum AndroidCopy {
    /// The query's own settings in the wording of Android's
    /// floorValidationProblem and validationProblem (SearchModels.kt) — the
    /// floors, the resin amount, an empty query, the depth and challenges —
    /// then the first problem the shared core finds with the requirements,
    /// in its own words: the stacks, combined levels, either/or groups and
    /// blankets are the core's to check and to word.
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
        return query.board.problems.first?.message
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
