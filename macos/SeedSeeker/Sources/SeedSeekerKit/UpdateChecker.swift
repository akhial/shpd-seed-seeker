import Foundation

/// A newer published release, as reported by the GitHub Releases API.
public struct UpdateInfo: Equatable, Sendable {
    public let version: String
    public let url: URL
}

public enum UpdateChecker {
    public static let releasesPage = URL(string: "https://github.com/akhial/shpd-seed-seeker/releases/latest")!
    private static let endpoint = URL(string: "https://api.github.com/repos/akhial/shpd-seed-seeker/releases/latest")!

    /// The `SEED_SEEKER_FAKE_LATEST` environment variable stands in for the
    /// latest release tag, bypassing the network and the daily throttle.
    public static var fakeLatest: String? {
        ProcessInfo.processInfo.environment["SEED_SEEKER_FAKE_LATEST"]
    }

    /// Returns the latest release when it is strictly newer than `current`;
    /// nil when up to date or on any network or parsing failure.
    public static func check(current: String) async -> UpdateInfo? {
        if let fake = fakeLatest { return newer(latest: fake, than: current, url: releasesPage) }
        var request = URLRequest(url: endpoint)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        guard let (data, response) = try? await URLSession.shared.data(for: request),
              (response as? HTTPURLResponse)?.statusCode == 200,
              let release = try? JSONDecoder().decode(Release.self, from: data) else { return nil }
        let url = release.htmlUrl.flatMap(URL.init(string:)) ?? releasesPage
        return newer(latest: release.tagName, than: current, url: url)
    }

    static func newer(latest: String, than current: String, url: URL) -> UpdateInfo? {
        guard let latestParts = parse(latest), let currentParts = parse(current),
              isOrderedAfter(latestParts, currentParts) else { return nil }
        return UpdateInfo(version: displayVersion(latest), url: url)
    }

    /// Strips the tag prefix and any pre-release suffix: "v1.2.3-beta" → "1.2.3".
    static func displayVersion(_ tag: String) -> String {
        var bare = Substring(tag.trimmingCharacters(in: .whitespaces))
        if bare.lowercased().hasPrefix("v") { bare = bare.dropFirst() }
        return String(bare.split(separator: "-", maxSplits: 1).first ?? "")
    }

    private static func parse(_ version: String) -> [Int]? {
        let parts = displayVersion(version).split(separator: ".").map { Int($0) }
        guard !parts.isEmpty, parts.allSatisfy({ $0 != nil }) else { return nil }
        return parts.compactMap(\.self)
    }

    private static func isOrderedAfter(_ lhs: [Int], _ rhs: [Int]) -> Bool {
        for index in 0..<max(lhs.count, rhs.count) {
            let left = index < lhs.count ? lhs[index] : 0
            let right = index < rhs.count ? rhs[index] : 0
            if left != right { return left > right }
        }
        return false
    }

    private struct Release: Decodable {
        let tagName: String
        let htmlUrl: String?
        private enum CodingKeys: String, CodingKey {
            case tagName = "tag_name", htmlUrl = "html_url"
        }
    }
}
