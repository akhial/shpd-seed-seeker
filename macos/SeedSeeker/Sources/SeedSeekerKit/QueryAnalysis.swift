import CSeedFinder
import Foundation

/// The same feasibility check and match estimate used by the web query panel.
public struct QueryAnalysis: Sendable {
    public let impossible: Bool
    public let probability: Double?

    public static func analyze(_ document: Data) throws -> QueryAnalysis {
        var probability = 0.0
        let code = document.withUnsafeBytes { bytes in
            seedfinder_analyze_query(bytes.bindMemory(to: UInt8.self).baseAddress,
                                     bytes.count, &probability)
        }
        guard code == 0 || code == 1 else {
            throw code == -1 ? SeedFinderEngineError.invalidArgument : SeedFinderEngineError.internalFailure
        }
        return QueryAnalysis(impossible: code == 1,
                             probability: probability.isFinite && probability > 0 ? probability : nil)
    }

    public var label: String {
        if impossible { return "Impossible query" }
        guard let probability else { return "Probability unavailable" }
        return "Match probability ≈ 1 in \(Self.denominator(1 / probability))"
    }

    private static func denominator(_ value: Double) -> String {
        // Match the web panel's compactNumber: two decimals below 100 units,
        // no trailing .00, and a space before the suffix.
        for (scale, suffix) in [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")] where value >= scale {
            let scaled = value / scale
            let number = String(format: scaled >= 100 ? "%.0f" : "%.2f", scaled)
            return "\(number.hasSuffix(".00") ? String(number.dropLast(3)) : number) \(suffix)"
        }
        return String(format: "%.0f", value)
    }
}
