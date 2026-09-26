import CSeedFinder
import Foundation

/// The engine's own constants, read once from `seedfinder_engine_info`.
///
/// Every value here is a fact about the linked Rust engine — the upstream game
/// version it targets, the bounds its validator applies, the game data its
/// generator uses — so the app reads them from the engine instead of keeping
/// mirrors that can drift.
public struct EngineInfo: Sendable {
    /// Upstream Shattered Pixel Dungeon version the engine targets.
    public let shpdVersion: String

    /// Upstream source commit tagged with the engine's target game version.
    public let shpdCommit: String

    /// Map coverage belongs to the linked engine, including supported boss floors.
    public let levelMapDepths: Set<Int>
    public let challengeNames: [Int: String]
    /// Largest portable results document the engine accepts, in bytes.
    public let resultsFileMaxBytes: Int

    /// The one instance, loaded on first use.
    public static let shared = load()

    /// Logical processors available to search workers, never less than one:
    /// the ceiling of the worker selector. Unlike the constants above this
    /// describes the host rather than the build, so it is read per call
    /// instead of being frozen into `shared`.
    public static var availableWorkers: Int { max(1, Int(seedfinder_available_workers())) }

    private static func load() -> EngineInfo {
        guard let packet = try? enginePacket({ out, length in
                  seedfinder_engine_info(out, length)
              }),
              let document = (try? JSONSerialization.jsonObject(with: packet)) as? [String: Any],
              let shpdVersion = document["shpdVersion"] as? String,
              let shpdCommit = document["shpdCommit"] as? String,
              let limits = document["limits"] as? [String: Any],
              let resultsFileMaxBytes = limits["resultsFileMaxBytes"] as? Int
        else {
            // The document is a constant of the statically linked engine, so
            // there is no runtime condition under which it can be missing.
            preconditionFailure("the linked engine returned no usable engine-info document")
        }
        let maps = document["levelMaps"] as? [String: Any]
        let challengeNames = Dictionary(uniqueKeysWithValues: (document["challenges"] as? [[String: Any]] ?? []).compactMap { entry -> (Int, String)? in
            guard let mask = entry["mask"] as? Int, let name = entry["name"] as? String else { return nil }
            return (mask, name)
        })
        return EngineInfo(shpdVersion: shpdVersion, shpdCommit: shpdCommit,
                          levelMapDepths: Set(maps?["supportedDepths"] as? [Int] ?? []), challengeNames: challengeNames,
                          resultsFileMaxBytes: resultsFileMaxBytes)
    }
}
