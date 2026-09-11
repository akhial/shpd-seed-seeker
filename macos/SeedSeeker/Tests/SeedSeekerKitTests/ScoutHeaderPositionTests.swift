import XCTest
@testable import SeedSeekerKit

final class ScoutHeaderPositionTests: XCTestCase {
    func testNoOverlayBeforeTheFirstHeaderReachesTheTop() {
        XCTAssertNil(ScoutHeaderPosition.pinned(in: [:]))
        XCTAssertNil(ScoutHeaderPosition.pinned(in: [1: header(y: 12)]))
    }

    func testFloorHeaderStaysPinnedWhileItsItemsScroll() {
        let position = ScoutHeaderPosition.pinned(in: [1: header(y: -200), 2: header(y: 80)])
        XCTAssertEqual(position?.depth, 1)
        XCTAssertEqual(position?.offset, 0)
    }

    func testNextHeaderPushesTheCurrentHeaderOffTheTop() {
        let position = ScoutHeaderPosition.pinned(in: [1: header(y: -200), 2: header(y: 10)])
        XCTAssertEqual(position?.depth, 1)
        XCTAssertEqual(position?.offset, -18)
    }

    func testHeaderChangesAtTheBoundaryAndSkipsAbsentFloors() {
        let position = ScoutHeaderPosition.pinned(in: [4: header(y: -400), 6: header(y: 0)])
        XCTAssertEqual(position?.depth, 6)
        XCTAssertEqual(position?.offset, 0)
    }

    func testLastHeaderStaysPinnedAndScrollingBackRestoresEarlierHeader() {
        let bottom = ScoutHeaderPosition.pinned(in: [22: header(y: -400), 24: header(y: -80)])
        XCTAssertEqual(bottom?.depth, 24)
        XCTAssertEqual(bottom?.offset, 0)
        let earlier = ScoutHeaderPosition.pinned(in: [22: header(y: -80), 24: header(y: 240)])
        XCTAssertEqual(earlier?.depth, 22)
        XCTAssertEqual(earlier?.offset, 0)
    }

    func testWrappedQuestHeaderUsesItsMeasuredHeight() {
        let position = ScoutHeaderPosition.pinned(in: [7: header(y: -100, height: 48), 8: header(y: 30)])
        XCTAssertEqual(position?.depth, 7)
        XCTAssertEqual(position?.offset, -18)
    }

    private func header(y: CGFloat, height: CGFloat = 28) -> CGRect {
        CGRect(x: 0, y: y, width: 400, height: height)
    }
}
