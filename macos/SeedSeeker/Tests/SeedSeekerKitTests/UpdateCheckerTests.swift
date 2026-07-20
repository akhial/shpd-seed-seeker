import Foundation
@testable import SeedSeekerKit
import XCTest

final class UpdateCheckerTests: XCTestCase {
    private let url = UpdateChecker.releasesPage

    func testNewerVersionIsReported() {
        XCTAssertEqual(UpdateChecker.newer(latest: "v0.6.0", than: "0.5.2", url: url),
                       UpdateInfo(version: "0.6.0", url: url))
        XCTAssertEqual(UpdateChecker.newer(latest: "v1.0.0", than: "0.5.2", url: url)?.version, "1.0.0")
        XCTAssertEqual(UpdateChecker.newer(latest: "0.5.10", than: "0.5.2", url: url)?.version, "0.5.10")
        XCTAssertEqual(UpdateChecker.newer(latest: "v0.6", than: "0.5.2", url: url)?.version, "0.6")
    }

    func testSameOrOlderVersionIsIgnored() {
        XCTAssertNil(UpdateChecker.newer(latest: "v0.5.2", than: "0.5.2", url: url))
        XCTAssertNil(UpdateChecker.newer(latest: "v0.5.1", than: "0.5.2", url: url))
        XCTAssertNil(UpdateChecker.newer(latest: "v0.5", than: "0.5.0", url: url))
    }

    func testSuffixesAndPrefixesAreStripped() {
        XCTAssertEqual(UpdateChecker.newer(latest: "v0.6.0-rc1", than: "0.5.2-demo", url: url)?.version, "0.6.0")
        XCTAssertEqual(UpdateChecker.displayVersion("v1.2.3-beta"), "1.2.3")
        XCTAssertEqual(UpdateChecker.displayVersion(" V2.0.0 "), "2.0.0")
    }

    func testGarbageIsIgnored() {
        XCTAssertNil(UpdateChecker.newer(latest: "nightly", than: "0.5.2", url: url))
        XCTAssertNil(UpdateChecker.newer(latest: "v9.9.9", than: "unknown", url: url))
        XCTAssertNil(UpdateChecker.newer(latest: "", than: "0.5.2", url: url))
    }
}
