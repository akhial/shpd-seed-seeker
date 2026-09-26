import AppKit
import SwiftUI
import XCTest
import SeedSeekerKit
@testable import SeedSeeker

@MainActor
final class LevelMapViewTests: XCTestCase {
    private func request(_ depth: Int = 13, branch: Int = 0, trinket: String? = nil) -> LevelMapRequest {
        LevelMapRequest(seed: "IPF-FCN-FZC", depth: depth, branch: branch,
                        challenges: 0, selectedTrinket: trinket)
    }

    private func press(_ key: String, in viewport: MapViewport) throws {
        let event = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero,
            modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil,
            characters: key, charactersIgnoringModifiers: key, isARepeat: false, keyCode: 0))
        viewport.keyDown(with: event)
    }

    func testItemInspectionRespectsCoordinatesAndClearsOnSecretsAndReload() async throws {
        let request = request(1)
        let bundle = try await LevelMapClient.shared.load(request)
        let map = bundle.document
        let tip = try XCTUnwrap(map.itemTooltips?.first { !$0.hidden })
        XCTAssertFalse(tip.items[0].description.isEmpty)
        let bounds = try XCTUnwrap(tip.bounds)
        XCTAssertTrue(map.itemTooltips!.flatMap(\.items).contains { $0.icon?.count == 4 })
        let spriteX = Double(tip.cell % map.width * 16 + bounds[0]) + Double(bounds[2]) / 2
        let spriteY = Double(tip.cell / map.width * 16 + bounds[1])
        XCTAssertEqual(map.itemAt(x: spriteX, y: spriteY + 0.5, secrets: false)?.cell, tip.cell)
        XCTAssertNotEqual(map.itemAt(x: spriteX, y: spriteY - 0.5, secrets: false)?.cell, tip.cell)
        let x = Double(tip.cell % map.width) * 16 + 8
        let y = Double(tip.cell / map.width) * 16 + 8
        XCTAssertEqual(map.itemAt(x: x, y: y, secrets: false)?.cell, tip.cell)
        XCTAssertNil(map.itemAt(x: -1, y: y, secrets: true))
        let viewport = MapViewport(frame: NSRect(x: 0, y: 0, width: map.pixelWidth + 16, height: map.pixelHeight + 16))
        viewport.update(bundle: bundle, request: request, time: 0)
        viewport.inspectItem(at: CGPoint(x: x + 8, y: y + 8))
        XCTAssertEqual(viewport.inspectedCell, tip.cell)
        XCTAssertEqual(viewport.subviews.count, 1)
        viewport.secrets = true
        XCTAssertNil(viewport.inspectedCell)
        try press("i", in: viewport)
        XCTAssertNotNil(viewport.inspectedCell)
        try press("+", in: viewport)
        XCTAssertNil(viewport.inspectedCell)
        viewport.update(bundle: nil, request: request, time: 10)
        XCTAssertTrue(viewport.subviews.isEmpty)
    }

    func testTrinketReloadKeepsZoomAndPanThroughLoadingAndRemoval() async throws {
        let original = request(), selected = request(trinket: "mimic_tooth")
        let map = try await LevelMapClient.shared.load(original)
        let changed = try await LevelMapClient.shared.load(selected)
        let viewport = MapViewport(frame: NSRect(x: 0, y: 0, width: 700, height: 600))
        viewport.update(bundle: map, request: original, time: 0)
        try press("+", in: viewport)
        try press("+", in: viewport)
        try press(String(UnicodeScalar(NSDownArrowFunctionKey)!), in: viewport)
        let zoom = viewport.zoom, pan = viewport.pan
        XCTAssertGreaterThan(zoom, 1)
        XCTAssertNotEqual(pan, .zero)

        for (request, bundle) in [(selected, changed), (original, map)] {
            viewport.update(bundle: nil, request: request, time: 100)
            XCTAssertFalse(viewport.hasMap)
            XCTAssertEqual(viewport.zoom, zoom)
            XCTAssertEqual(viewport.pan, pan)
            viewport.update(bundle: bundle, request: request, time: 200)
            XCTAssertTrue(viewport.hasMap)
            XCTAssertEqual(viewport.request, request)
            XCTAssertEqual(viewport.zoom, zoom)
            XCTAssertEqual(viewport.pan, pan)
        }
        XCTAssertTrue(viewport.acceptsFirstResponder)
        XCTAssertEqual(viewport.focusRingType, .none)
        try press("0", in: viewport)
        XCTAssertEqual(viewport.zoom, 1)
        XCTAssertEqual(viewport.pan, .zero)
    }

    func testChangingLocationStillFitsTheMap() async throws {
        let original = request()
        let map = try await LevelMapClient.shared.load(original)
        let viewport = MapViewport(frame: NSRect(x: 0, y: 0, width: 700, height: 600))
        for next in [request(17), request(branch: 1),
                     LevelMapRequest(seed: "AAA-AAA-AAA", depth: 13, challenges: 0, selectedTrinket: nil),
                     LevelMapRequest(seed: original.seed, depth: 13, challenges: 1, selectedTrinket: nil)] {
            viewport.update(bundle: map, request: original, time: 0)
            try press("+", in: viewport)
            try press(String(UnicodeScalar(NSDownArrowFunctionKey)!), in: viewport)
            viewport.update(bundle: nil, request: next, time: 100)
            XCTAssertEqual(viewport.zoom, 1)
            XCTAssertEqual(viewport.pan, .zero)
        }
    }

    func testPausedInlineMapLoadsAndKeepsItsViewportAcrossTrinketChanges() async throws {
        let window = host(depth: 11)
        defer { window.close() }
        let initial = try await loadedViewport(in: window, matching: request(11))
        try press("+", in: initial)
        let zoom = initial.zoom

        let host = try XCTUnwrap(window.contentView as? NSHostingView<LevelMapPanel>)
        host.rootView = panel(depth: 11, trinket: "mimic_tooth")
        let changed = try await loadedViewport(in: window, matching: request(11, trinket: "mimic_tooth"))
        XCTAssertTrue(initial === changed, "Loading must not recreate the viewport")
        XCTAssertEqual(changed.zoom, zoom)
    }

    func testSeparateMapPanelsDoNotShareFloor() async throws {
        let inline = host(depth: 13), expanded = host(depth: 13)
        defer { inline.close(); expanded.close() }
        let original = try await loadedViewport(in: inline, matching: request())
        let expandedHost = try XCTUnwrap(expanded.contentView as? NSHostingView<LevelMapPanel>)
        expandedHost.rootView = panel(depth: 17)
        let navigated = try await loadedViewport(in: expanded, matching: request(17))
        XCTAssertFalse(original === navigated)
        XCTAssertEqual(original.request?.depth, 13)
        XCTAssertEqual(original.accessibilityLabel(), "Floor 13 layout")
        XCTAssertEqual(navigated.accessibilityLabel(), "Floor 17 layout")
    }

    func testExpandedAreaAndSecretsSnapshotSurvivesTrinketChanges() async throws {
        let inline = host(depth: 17), expanded = host(depth: 17, initialBranch: 1, initialSecrets: true)
        defer { inline.close(); expanded.close() }
        let original = try await loadedViewport(in: inline, matching: request(17))
        let vault = try await loadedViewport(in: expanded, matching: request(17, branch: 1))
        XCTAssertTrue(vault.secrets)
        try press("+", in: vault)
        let zoom = vault.zoom

        let host = try XCTUnwrap(expanded.contentView as? NSHostingView<LevelMapPanel>)
        host.rootView = LevelMapPanel(
            world: ScoutWorld(seed: "IPF-FCN-FZC", items: [], selectedTrinket: "mimic_tooth"),
            depth: 17, challenges: 0, animated: false)
        let changed = try await loadedViewport(in: expanded, matching: request(17, branch: 1, trinket: "mimic_tooth"))
        XCTAssertTrue(vault === changed)
        XCTAssertTrue(changed.secrets)
        XCTAssertEqual(changed.zoom, zoom)
        XCTAssertEqual(original.request, request(17))
        XCTAssertFalse(original.secrets)
    }

    private func panel(depth: Int, trinket: String? = nil,
                       initialBranch: Int = 0, initialSecrets: Bool = false) -> LevelMapPanel {
        LevelMapPanel(world: ScoutWorld(seed: "IPF-FCN-FZC", items: [], selectedTrinket: trinket),
                      depth: depth, challenges: 0, animated: false, mapHeight: 260,
                      initialBranch: initialBranch, initialSecrets: initialSecrets)
    }

    private func host(depth: Int, initialBranch: Int = 0, initialSecrets: Bool = false) -> NSWindow {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 320),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: panel(depth: depth, initialBranch: initialBranch,
                                                         initialSecrets: initialSecrets))
        window.contentView?.layoutSubtreeIfNeeded()
        return window
    }

    private func loadedViewport(in window: NSWindow, matching request: LevelMapRequest) async throws -> MapViewport {
        func viewport(in view: NSView) -> MapViewport? {
            if let map = view as? MapViewport { return map }
            return view.subviews.lazy.compactMap { viewport(in: $0) }.first
        }
        for _ in 0..<100 {
            window.contentView?.layoutSubtreeIfNeeded()
            if let content = window.contentView, let map = viewport(in: content),
               map.hasMap, map.request == request { return map }
            try await Task.sleep(for: .milliseconds(50))
        }
        return try XCTUnwrap(Optional<MapViewport>.none, "Map did not load for floor \(request.depth)")
    }
}
