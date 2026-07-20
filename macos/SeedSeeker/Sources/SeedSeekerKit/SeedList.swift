import Foundation

public enum SeedListCodecError: Error, Equatable, LocalizedError {
    case invalidUTF8
    case invalidSeed(line: Int, value: String)
    case tooManySeeds(maximum: Int)

    public var errorDescription: String? {
        switch self {
        case .invalidUTF8:
            "Seed lists must be UTF-8 text"
        case .invalidSeed(let line, let value):
            "Invalid seed on line \(line): \(value)"
        case .tooManySeeds(let maximum):
            "Seed lists can contain at most \(maximum.formatted()) seeds"
        }
    }
}

/// Reads and writes interoperable seed lists containing one canonical seed per line.
public enum SeedListCodec {
    public static let maximumSeedCount = 1_024

    public static func decode(_ data: Data) throws -> [String] {
        guard var text = String(data: data, encoding: .utf8) else {
            throw SeedListCodecError.invalidUTF8
        }
        if text.first == "\u{feff}" { text.removeFirst() }

        var seeds: [String] = []
        var seen = Set<String>()
        let lines = text.replacingOccurrences(of: "\r\n", with: "\n")
            .components(separatedBy: "\n")
        for (offset, value) in lines.enumerated() {
            guard !value.trimmingCharacters(in: .whitespaces).isEmpty else { continue }
            guard let normalized = normalizedPortableSeed(value) else {
                throw SeedListCodecError.invalidSeed(line: offset + 1, value: value)
            }
            if seen.insert(normalized).inserted {
                seeds.append(normalized)
                guard seeds.count <= maximumSeedCount else {
                    throw SeedListCodecError.tooManySeeds(maximum: maximumSeedCount)
                }
            }
        }
        return seeds
    }

    public static func encode(_ seeds: [String]) throws -> Data {
        var normalizedSeeds: [String] = []
        var seen = Set<String>()
        for (offset, seed) in seeds.enumerated() {
            guard let normalized = normalizedPortableSeed(seed) else {
                throw SeedListCodecError.invalidSeed(line: offset + 1, value: seed)
            }
            if seen.insert(normalized).inserted { normalizedSeeds.append(normalized) }
        }
        guard normalizedSeeds.count <= maximumSeedCount else {
            throw SeedListCodecError.tooManySeeds(maximum: maximumSeedCount)
        }
        let text = normalizedSeeds.isEmpty ? "" : normalizedSeeds.joined(separator: "\n") + "\n"
        return Data(text.utf8)
    }

    private static func normalizedPortableSeed(_ value: String) -> String? {
        var bytes = Array(value.utf8)
        guard bytes.count == 11, bytes[3] == 45, bytes[7] == 45 else { return nil }
        for index in bytes.indices where index != 3 && index != 7 {
            switch bytes[index] {
            case 65...90:
                break
            case 97...122:
                bytes[index] -= 32
            default:
                return nil
            }
        }
        return String(decoding: bytes, as: UTF8.self)
    }
}
