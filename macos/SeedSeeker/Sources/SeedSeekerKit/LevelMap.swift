import CSeedFinder
import Foundation

/// A scene clock uses uptime independently of its reduced-motion sample. When
/// motion resumes, scheduled hazards begin at their captured first turn.
public struct LevelMapClock: Sendable {
    private var origin: Double
    private var wasReduced = false
    public init(now: Double) { origin = now }
    public mutating func elapsed(at now: Double, reducedMotion: Bool) -> Double {
        if wasReduced && !reducedMotion { origin = now }
        wasReduced = reducedMotion
        return reducedMotion ? 0 : max(0, now - origin)
    }
}

/// The resolved scout profile, independent of query edits and view state.
public struct LevelMapRequest: Hashable, Sendable {
    public let seed: String
    public let depth: Int
    public let branch: Int
    public let challenges: Int
    public let selectedTrinket: String?

    public init(seed: String, depth: Int, branch: Int = 0, challenges: Int, selectedTrinket: String?) {
        self.seed = seed; self.depth = depth; self.branch = branch
        self.challenges = challenges; self.selectedTrinket = selectedTrinket
    }

    /// Trinket changes refresh a scene without navigating away from its map.
    public func hasSameLocation(as other: Self) -> Bool {
        seed == other.seed && depth == other.depth && branch == other.branch && challenges == other.challenges
    }

    public func encoded() throws -> Data {
        guard (0...SearchLimits.challengeMask).contains(challenges) else { throw SeedFinderEngineError.invalidArgument }
        let names = EngineInfo.shared.challengeNames.filter { $0.key & challenges != 0 }.map(\.value).sorted()
        return try JSONSerialization.data(withJSONObject: [
            "seed": seed, "depth": depth, "branch": branch,
            "challenges": names,
            "trinket": selectedTrinket ?? "none",
        ], options: [.sortedKeys])
    }
}

/// Version 3 is a complete scene: contents, secrets and effects are drawn from
/// the selected layer stack without reconstructing any game rules in Swift.
public struct LevelMapDocument: Decodable, Sendable {
    public let format: String
    public let schemaVersion: Int
    public let assetRevision: String
    public let seed: String
    public let depth: Int
    public let branch: Int
    public let kind: String
    public let selectedTrinket: String?
    public let width: Int
    public let height: Int
    public let secretRooms: [[Int]]
    public let secretDoors: [Int]
    public let secretTraps: [Int]
    public let branches: [Branch]
    public let assets: [Asset]
    public let scene: Scene
    public let itemTooltips: [ItemTooltip]?

    public struct ItemTooltip: Decodable, Sendable {
        public let cell: Int
        public let label: String
        public let hidden: Bool
        public let items: [TooltipItem]
        public let bounds: [Int]?
    }
    public struct TooltipItem: Decodable, Sendable {
        public let name: String
        public let description: String
        public let image: Int
        public let quantity: Int
        public let deterministic: Bool
        public let icon: [Int]?
    }
    public func itemAt(x: Double, y: Double, secrets: Bool) -> ItemTooltip? {
        guard x >= 0, y >= 0, x < Double(pixelWidth), y < Double(pixelHeight) else { return nil }
        return itemTooltips?.first { tip in
            let bounds = tip.bounds ?? [0, 0, scene.tileSize, scene.tileSize]
            let left = Double((tip.cell % width) * scene.tileSize + bounds[0])
            let top = Double((tip.cell / width) * scene.tileSize + bounds[1])
            return (secrets || !tip.hidden) && x >= left && y >= top && x < left + Double(bounds[2]) && y < top + Double(bounds[3])
        }
    }

    public var secretCount: Int { secretRooms.count + secretDoors.count + secretTraps.count }
    public var pixelWidth: Int { width * scene.tileSize }
    public var pixelHeight: Int { height * scene.tileSize }

    public struct Branch: Decodable, Sendable, Identifiable {
        public let depth: Int
        public let branch: Int
        public let kind: String
        public var id: Int { branch }
        public var label: String { kind == "imp_vault" ? "Imp Vault" : "Blacksmith Mine" }
    }
    public struct Asset: Decodable, Sendable {
        public let id: String
        public let width: Int
        public let height: Int
        public let sha256: String
    }
    public struct Scene: Decodable, Sendable {
        public let tileSize: Int
        public let sprites: [Sprite]
        public let layers: [Layer]
        public let concealedLayers: [Layer]
        public let emitters: [Emitter]
        public let concealedEmitters: [Emitter]
    }
    public struct Layer: Decodable, Sendable {
        public let name: String
        public let blend: String?
        public let cells: [Int?]
    }
    public struct Sprite: Decodable, Sendable {
        public let frameDurationMs: Double
        public let frames: [[Draw]]
        public func frameIndex(at elapsed: Double) -> Int {
            Int(max(0, elapsed) / frameDurationMs) % frames.count
        }
    }
    public struct Draw: Decodable, Hashable, Sendable {
        public let kind: String
        public let destination: [Double]
        public let asset: String?
        public let source: [Double]?
        public let opacity: Double?
        public let tint: [Double]?
        public let glow: Glow?
        public let rgba: [Double]?
    }
    public struct Glow: Decodable, Hashable, Sendable {
        public let color: [Double]
        public let periodMs: Double
        public func strength(at elapsed: Double) -> Double {
            let phase = max(0, elapsed).truncatingRemainder(dividingBy: periodMs * 2) / periodMs
            return min(phase, 2 - phase) * 0.6
        }
    }
    public struct Curve: Decodable, Sendable {
        public let points: [[Double]]
        public let sqrt: Bool
        public func value(at progress: Double) -> Double {
            let p = min(1, max(0, progress)) * 1000
            let value: Double
            if let right = points.firstIndex(where: { $0[0] >= p }) {
                if right == 0 { value = points[0][1] }
                else {
                    let a = points[right - 1], b = points[right]
                    value = a[1] + (b[1] - a[1]) * (p - a[0]) / (b[0] - a[0])
                }
            } else { value = points.last![1] }
            return sqrt ? Foundation.sqrt(max(0, value / 1000)) : value / 1000
        }
    }
    public struct Particle: Decodable, Sendable {
        public let birthMs: Double
        public let lifespanMs: Double
        public let position: [Double]
        public let scale: Double
        public let angle: Double
    }
    public struct ParticleState: Sendable {
        public let x: Double, y: Double, scale: Double, scaleX: Double, scaleY: Double, alpha: Double, angle: Double
    }
    public struct Emitter: Decodable, Sendable {
        public let startMs: Double?
        public let wallMask: Bool?
        public let clipToChasm: Bool?
        public let cell: Int
        public let loopMs: Double
        public let blend: String?
        public let image: Draw
        public let velocity: [Double]
        public let acceleration: [Double]
        public let angularSpeed: Double
        public let alpha: Curve
        public let scale: Curve
        public let scaleX: Curve?
        public let scaleY: Curve?
        public let particles: [Particle]

        public func state(of particle: Particle, at elapsed: Double) -> ParticleState? {
            let clock = max(0, elapsed) - (startMs ?? 0)
            if startMs != nil && clock < particle.birthMs { return nil }
            let age = ((clock - particle.birthMs).truncatingRemainder(dividingBy: loopMs) + loopMs)
                .truncatingRemainder(dividingBy: loopMs)
            guard age < particle.lifespanMs else { return nil }
            let seconds = age / 1000, progress = age / particle.lifespanMs
            return ParticleState(
                x: particle.position[0] / 1000 + velocity[0] * seconds + acceleration[0] * seconds * seconds / 2,
                y: particle.position[1] / 1000 + velocity[1] * seconds + acceleration[1] * seconds * seconds / 2,
                scale: particle.scale / 1000 * scale.value(at: progress),
                scaleX: scaleX?.value(at: progress) ?? 1,
                scaleY: scaleY?.value(at: progress) ?? 1, alpha: alpha.value(at: progress),
                angle: (particle.angle + angularSpeed * seconds) * .pi / 180)
        }
    }

    public static func decode(_ data: Data) throws -> Self {
        let map = try JSONDecoder().decode(Self.self, from: data)
        guard map.format == "seed-seeker-level-map", map.schemaVersion == 3,
              (1...256).contains(map.width), (1...256).contains(map.height), map.scene.tileSize == 16
        else { throw SeedFinderEngineError.invalidResponse }
        let count = map.width * map.height
        guard
              (map.scene.layers + map.scene.concealedLayers).allSatisfy({ layer in
                  layer.cells.count == count && layer.cells.allSatisfy { $0 == nil || map.scene.sprites.indices.contains($0!) }
              }), map.scene.sprites.allSatisfy({ $0.frameDurationMs > 0 && !$0.frames.isEmpty })
        else { throw SeedFinderEngineError.invalidResponse }
        let assetIDs = Set(map.assets.map(\.id))
        func validDraw(_ draw: Draw) -> Bool {
            guard draw.destination.count == 4 else { return false }
            if draw.kind == "fill" { return draw.rgba?.count == 4 }
            return draw.kind == "blit" && assetIDs.contains(draw.asset ?? "") && draw.source?.count == 4
                && (draw.tint == nil || draw.tint?.count == 3)
                && (draw.glow == nil || (draw.glow!.color.count == 3 && draw.glow!.periodMs > 0))
        }
        func validCurve(_ curve: Curve) -> Bool {
            !curve.points.isEmpty && curve.points.allSatisfy { $0.count == 2 }
                && zip(curve.points, curve.points.dropFirst()).allSatisfy { $0[0] < $1[0] }
        }
        guard map.scene.sprites.allSatisfy({ $0.frames.allSatisfy { $0.allSatisfy(validDraw) } }),
              (map.scene.emitters + map.scene.concealedEmitters).allSatisfy({ emitter in
                  (0..<count).contains(emitter.cell) && emitter.loopMs > 0 && validDraw(emitter.image)
                      && emitter.velocity.count == 2 && emitter.acceleration.count == 2
                      && validCurve(emitter.alpha) && validCurve(emitter.scale)
                      && (emitter.scaleX == nil || validCurve(emitter.scaleX!))
                      && (emitter.scaleY == nil || validCurve(emitter.scaleY!))
                      && emitter.particles.allSatisfy { $0.position.count == 2 && $0.lifespanMs > 0 }
              }) else { throw SeedFinderEngineError.invalidResponse }
        return map
    }
}

public struct LevelMapBundle: Sendable {
    public let document: LevelMapDocument
    public let assets: [String: Data]
}

/// Native generation and asset reads execute on this actor, away from the UI.
/// One engine is statically linked for the process lifetime; its revision is
/// immutable. PNGs are shared by content digest, maps use an eight-entry LRU.
public actor LevelMapClient {
    public static let shared = LevelMapClient()
    private var maps: [LevelMapRequest: LevelMapBundle] = [:]
    private var order: [LevelMapRequest] = []
    private var assets: [String: Data] = [:]

    public func load(_ request: LevelMapRequest) throws -> LevelMapBundle {
        try Task.checkCancellation()
        if let cached = maps[request] {
            order.removeAll { $0 == request }; order.append(request)
            return cached
        }
        let input = try request.encoded()
        let packet = try enginePacket { out, length in
            input.withUnsafeBytes { bytes in
                seedfinder_level_map(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, out, length)
            }
        }
        let map = try LevelMapDocument.decode(packet)
        var images: [String: Data] = [:]
        for asset in map.assets {
            if assets[asset.sha256] == nil {
                let id = Data(asset.id.utf8)
                assets[asset.sha256] = try enginePacket { out, length in
                    id.withUnsafeBytes { bytes in
                        seedfinder_level_map_asset(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, out, length)
                    }
                }
            }
            images[asset.id] = assets[asset.sha256]
        }
        let bundle = LevelMapBundle(document: map, assets: images)
        maps[request] = bundle; order.append(request)
        while order.count > 8 { maps.removeValue(forKey: order.removeFirst()) }
        return bundle
    }
}

/// Match witnesses select an option for a whole group, including all its items.
/// Unmatched items in the selected option must stay readable.
public struct ScoutChoiceStatus: Sendable {
    private let selected: [Int: Int]
    public init(items: [ScoutItem], matched: Set<Int>) {
        var choices: [Int: Int] = [:]
        for (index, item) in items.enumerated() where matched.contains(index) {
            if case let .choice(group, option) = item.accessibility { choices[group] = option }
        }
        selected = choices
    }
    public func isDimmed(_ accessibility: ScoutAccessibility, matched: Bool) -> Bool {
        guard !matched, case let .choice(group, option) = accessibility, let chosen = selected[group] else { return false }
        return chosen != option
    }
    public static func letter(_ group: Int) -> String {
        String(UnicodeScalar(65 + max(0, group) % 26)!)
    }
}
