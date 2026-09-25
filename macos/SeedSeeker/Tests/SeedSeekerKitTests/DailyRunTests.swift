import XCTest
@testable import SeedSeekerKit

final class DailyRunTests: XCTestCase {
    func testDailyDatesStayOutsideSearchCodesAndRoundTripThroughNativeScout() async throws {
        let date = "2026-09-25"
        XCTAssertEqual(SeedCode.parse(date)?.value, 7_219_798_078_976)
        XCTAssertTrue(SeedCode.isScoutable(date))
        XCTAssertFalse(SeedCode.isCanonical(date))
        XCTAssertFalse(SeedCode.isScoutable("2026-02-29"))
        let world = try await ProductionSeedFinderEngine().scoutSeed(date, challenges: 0)
        XCTAssertEqual(world.seed, date)
        XCTAssertFalse(world.items.isEmpty)
        XCTAssertEqual(world.itemMappings?.scrolls.first?.appearance, "ODAL")
    }

    func testPickerDatesUseUTCAtTheDayBoundary() {
        let midnight = DailyRunDate.date("2026-09-25")!
        XCTAssertEqual(DailyRunDate.code(midnight.addingTimeInterval(-1)), "2026-09-24")
        XCTAssertEqual(DailyRunDate.code(midnight), "2026-09-25")
        XCTAssertEqual(DailyRunDate.code(midnight.addingTimeInterval(86399)), "2026-09-25")
    }
}
