import Foundation

/// A sticky header follows the visible floor until the next header pushes it
/// out. Its placement is independent of the document's layout and row heights.
public struct ScoutHeaderPosition: Equatable, Sendable {
    public let depth: Int
    public let offset: CGFloat

    public static func pinned(in headers: [Int: CGRect]) -> ScoutHeaderPosition? {
        let depths = headers.keys.sorted()
        guard let depth = depths.last(where: { headers[$0]!.minY <= 0 }),
              let frame = headers[depth], frame.height > 0 else { return nil }
        let next = depths.first(where: { $0 > depth }).flatMap { headers[$0] }
        return ScoutHeaderPosition(depth: depth, offset: min(0, (next?.minY ?? frame.height) - frame.height))
    }
}
