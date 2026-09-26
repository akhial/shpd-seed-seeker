import Foundation

/// Private storage, separate from the deliberately capped, portable results export.
struct PendingSearchCheckpoint: Codable {
    let request: SearchRequest
    let workers: Int
    var resultGoal: Int?
}

struct SearchCheckpoint: Codable {
    static var engineIdentifier: String {
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? ""
        return build + ":" + EngineInfo.shared.shpdVersion + ":" + EngineInfo.shared.shpdCommit
    }
    var schema = 1
    var engine = Self.engineIdentifier
    var results: [SeedResult]
    var collected: [SeedResult]
    var query: SavedQuery?
    var target: TargetState?
    var baseRun: BaseRun?
    var pending: PendingSearchCheckpoint?
    var state: SearchState?
    var scanned: Int64
    var total: Int64
    var elapsed: TimeInterval
    var probability: Double?
    var isImported: Bool
    var importedDropped: Int
    var errorCode: Int64
    var message: String?

    static func load(from url: URL) throws -> SearchCheckpoint? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        let saved = try JSONDecoder().decode(Self.self, from: Data(contentsOf: url))
        guard saved.schema == 1 else {
            throw NSError(domain: "SeedSeeker", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "Unsupported saved search format."])
        }
        return saved
    }

    func save(to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(self).write(to: url, options: .atomic)
    }
}
