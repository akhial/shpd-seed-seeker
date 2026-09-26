// SPDX-License-Identifier: GPL-3.0-or-later
import Foundation
import Observation
import SeedSeekerKit

/// A scout keeps the exact query and recipe that produced its visible world.
/// Changing the Finder board cannot silently change a map or trinket preview.
@MainActor @Observable
final class ScoutModel {
    var input = ""
    private(set) var world: ScoutWorld?
    private(set) var error: String?
    private(set) var loading = false
    private(set) var requestedSeed: String?
    private(set) var renderedQuery: SearchRequest?
    private(set) var renderedChallenges = 0
    private(set) var matches: ScoutMatches?
    private var generation = 0
    private let engine = ProductionSeedFinderEngine()

    func scout(seed: String, query: SearchRequest?, result: SeedResult? = nil, challenges: Int = 0) {
        generate(seed: seed, query: query, challenges: query?.challenges ?? challenges,
                 trinket: result.map { $0.selectedTrinket ?? "none" })
    }

    func editInput(_ value: String) {
        input = SeedCode.formatInput(value)
        if input != world?.seed { world = nil; matches = nil }
        error = nil
    }

    func selectTrinket(_ trinket: String) {
        guard let world, !loading else { return }
        generate(seed: world.seed, query: renderedQuery, challenges: renderedChallenges,
                 trinket: trinket)
    }

    private func generate(seed: String, query: SearchRequest?, challenges: Int, trinket: String?) {
        input = SeedCode.formatInput(seed)
        guard SeedCode.isScoutable(input) else {
            error = "Choose a daily date or enter a XXX-XXX-XXX seed"
            return
        }
        let requested = input
        requestedSeed = requested
        loading = true
        error = nil
        if world?.seed != requested { world = nil; matches = nil }
        generation += 1
        let current = generation
        Task {
            do {
                let request = try ScoutCodec.encodeRequest(seed: requested, challenges: challenges,
                                                          query: query, trinket: trinket)
                let generated = try await engine.scoutSeed(requested, challenges: challenges,
                                                           query: query, trinket: trinket)
                let marked: ScoutMatches?
                if let query {
                    marked = try await Task.detached {
                        try ScoutMatches.mark(request, query: QueryDocument.encode(query))
                    }.value
                } else { marked = nil }
                guard generation == current else { return }
                world = generated
                matches = marked
                renderedQuery = query
                renderedChallenges = challenges
            } catch {
                guard generation == current else { return }
                self.error = "The native scout could not generate this seed."
                requestedSeed = world?.seed
            }
            loading = false
        }
    }
}
