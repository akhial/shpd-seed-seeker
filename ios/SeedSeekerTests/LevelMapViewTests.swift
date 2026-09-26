// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import UIKit
import XCTest
@testable import SeedSeeker

@MainActor final class LevelMapViewTests: XCTestCase {
    private let request = LevelMapRequest(seed: "AAA-AAA-AAA", depth: 7, challenges: 0, selectedTrinket: nil)

    private func update(_ viewport: MapViewport, with bundle: LevelMapBundle?,
                        request: LevelMapRequest? = nil, secrets: Bool = false) {
        viewport.update(bundle: bundle, request: request ?? self.request, secrets: secrets,
                        animated: false, reduceMotion: true, expanded: true,
                        label: "Floor 7 layout", navigate: { _ in })
    }

    private func spritePoint(_ tip: LevelMapDocument.ItemTooltip, map: LevelMapDocument) -> CGPoint {
        let bounds = tip.bounds ?? [0, 0, 16, 16]
        return CGPoint(x: Double(tip.cell % map.width * 16 + bounds[0]) + Double(bounds[2]) / 2,
                       y: Double(tip.cell / map.width * 16 + bounds[1]) + 0.5)
    }

    func testRaisedSpriteInspectionAndTransparentOverlayDismissal() async throws {
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { !$0.hidden && ($0.bounds?[1] ?? 0) < 0 })
        let point = spritePoint(tip, map: map)
        let viewport = MapViewport(frame: CGRect(x: 0, y: 0, width: map.pixelWidth, height: map.pixelHeight))
        update(viewport, with: bundle)
        viewport.inspectItem(at: point)
        XCTAssertEqual(viewport.inspectedCell, tip.cell, "The raised artwork above the cell must remain tappable")
        let overlay = try XCTUnwrap(viewport.subviews.first as? MapItemOverlayHost)
        let card = try XCTUnwrap(overlay.presentation.card)
        XCTAssertTrue(viewport.bounds.contains(card.frame))
        XCTAssertGreaterThan(card.frame.height, 40)
        XCTAssertNil(overlay.hitTest(CGPoint(x: 1, y: 1), with: nil), "The transparent area must pass map gestures through")

        viewport.inspectItem(at: CGPoint(x: -1, y: -1), toggleSelection: true)
        XCTAssertNil(viewport.inspectedCell)
        XCTAssertNil(overlay.hitTest(card.frame.origin, with: nil), "Outgoing glass must release its hit target immediately")
        viewport.inspectItem(at: point)
        XCTAssertEqual(viewport.inspectedCell, tip.cell)
        XCTAssertTrue(viewport.subviews.first === overlay, "Re-entry should reuse the glass transition's container")
        viewport.inspectItem(at: point, toggleSelection: true)
        XCTAssertNil(viewport.inspectedCell, "Tapping the selected item dismisses inspection")
    }

    func testZoomSecretsAndMapChangesDismissInspectionWithoutLosingZoomOnTrinketReload() async throws {
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { !$0.hidden })
        let point = spritePoint(tip, map: map)
        let viewport = MapViewport(frame: CGRect(x: 0, y: 0, width: map.pixelWidth, height: map.pixelHeight))
        update(viewport, with: bundle)
        viewport.inspectItem(at: point)
        viewport.accessibilityIncrement()
        XCTAssertNil(viewport.inspectedCell)
        XCTAssertGreaterThan(viewport.zoom, 1)
        let zoom = viewport.zoom

        let trinket = LevelMapRequest(seed: request.seed, depth: request.depth, challenges: 0, selectedTrinket: "mimic_tooth")
        update(viewport, with: nil, request: trinket)
        XCTAssertNil(viewport.inspectedCell)
        XCTAssertEqual(viewport.zoom, zoom, "A trinket refresh preserves the map viewport")

        let next = LevelMapRequest(seed: request.seed, depth: 8, challenges: 0, selectedTrinket: nil)
        update(viewport, with: nil, request: next)
        XCTAssertEqual(viewport.zoom, 1)
        update(viewport, with: bundle)
        viewport.inspectItem(at: point)
        XCTAssertEqual(viewport.inspectedCell, tip.cell)
        update(viewport, with: bundle, secrets: true)
        XCTAssertNil(viewport.inspectedCell)
        viewport.inspectItem(at: point)
        XCTAssertTrue(viewport.accessibilityPerformEscape())
        XCTAssertNil(viewport.inspectedCell)
    }

    func testHiddenItemsCannotBeInspectedUntilSecretsAreEnabled() async throws {
        // The secret chest room on floor 2 contains this locked chest and its
        // generated Kunai. Floor 1 has no concealed item tooltips in this seed.
        let request = LevelMapRequest(seed: "AAA-AAA-AAA", depth: 2, challenges: 0, selectedTrinket: nil)
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { $0.items.contains { $0.name == "Kunai" } })
        XCTAssertTrue(tip.hidden)
        XCTAssertEqual(tip.label, "Locked Chest")
        let viewport = MapViewport(frame: CGRect(x: 0, y: 0, width: map.pixelWidth, height: map.pixelHeight))
        let point = spritePoint(tip, map: map)
        update(viewport, with: bundle, request: request)
        viewport.inspectItem(at: point)
        XCTAssertNil(viewport.inspectedCell)
        update(viewport, with: bundle, request: request, secrets: true)
        viewport.inspectItem(at: point)
        XCTAssertEqual(viewport.inspectedCell, tip.cell)
        update(viewport, with: bundle, request: request)
        XCTAssertNil(viewport.inspectedCell)
    }

    func testInspectionStaysInsidePartiallyClippedMap() async throws {
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { !$0.hidden })
        let point = spritePoint(tip, map: map)
        let viewport = MapViewport(frame: CGRect(x: 0, y: 40 - point.y,
                                                width: CGFloat(map.pixelWidth), height: CGFloat(map.pixelHeight)))
        let clip = UIView(frame: CGRect(x: 0, y: 0, width: map.pixelWidth, height: 160))
        clip.clipsToBounds = true
        clip.addSubview(viewport)
        update(viewport, with: bundle)

        for height: CGFloat in [160, 72] {
            clip.frame.size.height = height
            viewport.inspectItem(at: point)
            let overlay = try XCTUnwrap(viewport.subviews.first as? MapItemOverlayHost)
            let card = try XCTUnwrap(overlay.presentation.card)
            let visible = viewport.bounds.intersection(viewport.convert(clip.bounds, from: clip))
            XCTAssertTrue(visible.contains(card.frame), "The full card, including Close, must stay inside the visible map")
            XCTAssertGreaterThanOrEqual(card.frame.height, 48, "The compact card still leaves room for its fixed Close control")
            XCTAssertGreaterThan(card.frame.minY, visible.minY)
            viewport.dismissInspection(animated: false)
        }
    }

    func testVoiceOverInspectionAvoidsPinnedHeaderWhenMapCenterIsClipped() async throws {
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { !$0.hidden })
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let originalKeyWindow = scene.windows.first { $0.isKeyWindow }
        let window = UIWindow(windowScene: scene)
        window.frame = scene.coordinateSpace.bounds
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        window.layoutIfNeeded()
        defer {
            window.isHidden = true
            window.rootViewController = nil
            originalKeyWindow?.makeKey()
        }

        let clip = UIView(frame: CGRect(x: 16, y: max(80, window.safeAreaInsets.top + 20),
                                        width: window.bounds.width - 32, height: 200))
        clip.clipsToBounds = true
        controller.view.addSubview(clip)
        let viewport = MapViewport(frame: CGRect(x: 0, y: 220 - CGFloat(map.pixelHeight),
                                                width: clip.bounds.width, height: CGFloat(map.pixelHeight)))
        clip.addSubview(viewport)
        let pinnedHeader = UIView(frame: CGRect(x: 0, y: 0, width: clip.bounds.width, height: 120))
        pinnedHeader.backgroundColor = .black
        clip.addSubview(pinnedHeader)
        update(viewport, with: bundle)
        window.layoutIfNeeded()

        let clippedCenter = CGPoint(x: viewport.bounds.midX, y: viewport.bounds.midY)
        XCTAssertTrue(viewport.inspectionVisibleBounds(around: clippedCenter).isNull)
        let coveredPoint = viewport.convert(CGPoint(x: clip.bounds.midX, y: 100), from: clip)
        XCTAssertTrue(viewport.inspectionVisibleBounds(around: coveredPoint).isNull,
                      "Pointer placement remains strict when a sibling covers the point")

        let name = tip.items.map(\.name).joined(separator: ", ")
        let action = try XCTUnwrap(viewport.accessibilityCustomActions?.first { $0.name == name })
        let handler = try XCTUnwrap(action.actionHandler)
        XCTAssertTrue(handler(action), "VoiceOver must find an uncovered anchor instead of the clipped map center")
        XCTAssertEqual(viewport.inspectedCell, tip.cell)
        let overlay = try XCTUnwrap(viewport.subviews.first as? MapItemOverlayHost)
        let card = try XCTUnwrap(overlay.presentation.card)
        let visibleFrame = clip.convert(card.frame, from: viewport)
        XCTAssertTrue(clip.bounds.contains(visibleFrame))
        XCTAssertGreaterThan(visibleFrame.minY, pinnedHeader.frame.maxY)
        viewport.dismissInspection(animated: false)
    }
}
