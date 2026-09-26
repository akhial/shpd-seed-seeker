// SPDX-License-Identifier: GPL-3.0-or-later
import ImageIO
import Observation
import SeedSeekerKit
import SwiftUI
import UIKit

/// Each disclosed floor retains its own map. The expanded map has independent
/// floor, branch, secret and viewport state, while trinkets belong to the Scout.
struct LevelMapView: View {
    let world: ScoutWorld
    let depth: Int
    let floors: [Int]
    let challenges: Int
    var active = true
    let changingTrinket: Bool
    let onSelectTrinket: (String) -> Void
    @State private var expandedMap: ExpandedMapSelection?

    var body: some View {
        LevelMapPanel(world: world, depth: depth, challenges: challenges,
                      animated: active && expandedMap == nil, mapHeight: 280,
                      onExpand: { branch, secrets in
                          expandedMap = ExpandedMapSelection(world: world, floors: floors,
                                                             changingTrinket: changingTrinket,
                                                             branch: branch, secrets: secrets)
                      })
        .fullScreenCover(item: $expandedMap) { selection in
            ExpandedLevelMapView(selection: selection, initialDepth: depth,
                                 challenges: challenges,
                                 onSelectTrinket: onSelectTrinket)
        }
        .onChange(of: world.selectedTrinket) { _, _ in expandedMap?.world = world }
        .onChange(of: changingTrinket) { _, changing in
            expandedMap?.world = world
            expandedMap?.changingTrinket = changing
        }
        .onChange(of: floors) { _, floors in expandedMap?.floors = floors }
        .onChange(of: world.seed) { _, _ in expandedMap = nil }
        .onChange(of: challenges) { _, _ in expandedMap = nil }
    }
}

/// Presentation identity stays stable while the shared Scout produces another
/// trinket preview. The presented view observes this reference directly rather
/// than retaining the world value captured when the cover first opened.
@MainActor @Observable private final class ExpandedMapSelection: Identifiable {
    let id = UUID()
    var world: ScoutWorld
    var floors: [Int]
    var changingTrinket: Bool
    let branch: Int
    let secrets: Bool

    init(world: ScoutWorld, floors: [Int], changingTrinket: Bool, branch: Int, secrets: Bool) {
        self.world = world
        self.floors = floors
        self.changingTrinket = changingTrinket
        self.branch = branch
        self.secrets = secrets
    }
}

private struct ExpandedLevelMapView: View {
    let selection: ExpandedMapSelection
    let initialDepth: Int
    let challenges: Int
    let onSelectTrinket: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var selectedDepth: Int?

    private var depth: Int { selectedDepth ?? initialDepth }

    var body: some View {
        LevelMapPanel(world: selection.world, depth: depth, challenges: challenges,
                      animated: true, expanded: true, floors: selection.floors,
                      changingTrinket: selection.changingTrinket, onSelectTrinket: onSelectTrinket,
                      onClose: { dismiss() }, navigate: navigate,
                      initialBranch: selection.branch, initialSecrets: selection.secrets)
            .background(AppTheme.background)
    }

    private func navigate(_ delta: Int) {
        guard let index = selection.floors.firstIndex(of: depth),
              selection.floors.indices.contains(index + delta) else { return }
        selectedDepth = selection.floors[index + delta]
    }
}

/// Load identity includes the trinket; location identity does not. Changing a
/// trinket refreshes branch availability and scenery without losing zoom/pan.
struct LevelMapPanel: View {
    let world: ScoutWorld
    let depth: Int
    let challenges: Int
    let animated: Bool
    var mapHeight: CGFloat?
    var onExpand: ((Int, Bool) -> Void)?
    var expanded: Bool
    var floors: [Int]
    var changingTrinket: Bool
    var onSelectTrinket: (String) -> Void
    var onClose: () -> Void
    var navigate: (Int) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.scenePhase) private var scenePhase
    @State private var branchSelection: (profile: LevelMapRequest, branch: Int)?
    @State private var secrets: Bool
    @State private var bundle: LevelMapBundle?
    @State private var loadedKey: LevelMapRequest?
    @State private var parentKey: LevelMapRequest?
    @State private var parentBranches: [LevelMapDocument.Branch] = []
    @State private var error: String?
    @State private var retry = 0

    init(world: ScoutWorld, depth: Int, challenges: Int, animated: Bool,
         mapHeight: CGFloat? = nil, onExpand: ((Int, Bool) -> Void)? = nil,
         expanded: Bool = false, floors: [Int] = [], changingTrinket: Bool = false,
         onSelectTrinket: @escaping (String) -> Void = { _ in },
         onClose: @escaping () -> Void = {}, navigate: @escaping (Int) -> Void = { _ in },
         initialBranch: Int = 0, initialSecrets: Bool = false) {
        self.world = world
        self.depth = depth
        self.challenges = challenges
        self.animated = animated
        self.mapHeight = mapHeight
        self.onExpand = onExpand
        self.expanded = expanded
        self.floors = floors
        self.changingTrinket = changingTrinket
        self.onSelectTrinket = onSelectTrinket
        self.onClose = onClose
        self.navigate = navigate
        let profile = LevelMapRequest(seed: world.seed, depth: depth, challenges: challenges,
                                      selectedTrinket: world.selectedTrinket)
        _branchSelection = State(initialValue: (profile, initialBranch))
        _secrets = State(initialValue: initialSecrets)
    }

    private var profile: LevelMapRequest {
        LevelMapRequest(seed: world.seed, depth: depth, challenges: challenges,
                        selectedTrinket: world.selectedTrinket)
    }
    private var location: LevelMapRequest {
        LevelMapRequest(seed: world.seed, depth: depth, challenges: challenges, selectedTrinket: nil)
    }
    private var branch: Int {
        guard let branchSelection, branchSelection.profile.hasSameLocation(as: profile) else { return 0 }
        return branchSelection.branch
    }
    private var request: LevelMapRequest {
        LevelMapRequest(seed: world.seed, depth: depth, branch: branch,
                        challenges: challenges, selectedTrinket: world.selectedTrinket)
    }
    private var map: LevelMapBundle? { loadedKey == request ? bundle : nil }
    private var branches: [LevelMapDocument.Branch] {
        parentKey?.hasSameLocation(as: profile) == true ? parentBranches : []
    }
    private var title: String {
        branch == 0 ? "Floor \(depth) layout" : (map?.document.kind == "imp_vault" ? "Imp Vault" : "Blacksmith Mine")
    }
    private var floorIndex: Int { floors.firstIndex(of: depth) ?? 0 }
    private struct LoadKey: Hashable { let request: LevelMapRequest; let retry: Int }

    var body: some View {
        VStack(spacing: 0) {
            if expanded {
                ScoutFloorHeading(depth: depth, world: world, onCloseMap: onClose)
                    .padding(.horizontal, 16).padding(.vertical, 4)
                HStack(spacing: 12) {
                    trinketShortcuts
                    if branches.isEmpty {
                        Spacer(minLength: 0)
                        secretToggle
                    }
                }
                .padding(.horizontal, 16).padding(.vertical, 6)
                if !branches.isEmpty { toolbar }
            } else {
                toolbar
            }
            stage
                .frame(height: mapHeight)
                .frame(maxHeight: expanded ? .infinity : nil)
        }
        .background(expanded ? AppTheme.background : AppTheme.surface)
        .clipShape(RoundedRectangle(cornerRadius: expanded ? 0 : 20))
        .onChange(of: location) { _, _ in branchSelection = nil }
        .task(id: LoadKey(request: request, retry: retry)) {
            let requested = request, parent = profile
            error = nil
            do {
                if parentKey != parent {
                    let main = try await LevelMapClient.shared.load(parent)
                    guard !Task.isCancelled, requested == request else { return }
                    parentKey = parent
                    parentBranches = main.document.branches
                    if requested.branch == 0 {
                        bundle = main
                        loadedKey = requested
                        return
                    }
                    if !parentBranches.contains(where: { $0.branch == requested.branch }) {
                        branchSelection = nil
                        return
                    }
                }
                let next = try await LevelMapClient.shared.load(requested)
                guard !Task.isCancelled, requested == request else { return }
                bundle = next
                loadedKey = requested
            } catch {
                guard !Task.isCancelled, requested == request else { return }
                self.error = error.localizedDescription
            }
        }
    }

    private var trinketShortcuts: some View {
        HStack(spacing: 6) {
            ForEach(Array(world.trinketOrder.prefix(4))) { item in
                let selected = world.selectedTrinket == item.id
                Button { onSelectTrinket(selected ? "none" : item.id) } label: {
                    ItemSpriteView(item: item, pointSize: 22)
                        .frame(width: 38, height: 38)
                        .background(selected ? Color.accentColor.opacity(0.14) : .clear,
                                    in: RoundedRectangle(cornerRadius: 10))
                        .overlay(RoundedRectangle(cornerRadius: 10)
                            .strokeBorder(selected ? Color.accentColor : Color.secondary.opacity(0.25),
                                          lineWidth: selected ? 2 : 1))
                }
                .buttonStyle(.plain)
                .disabled(changingTrinket)
                .accessibilityLabel(item.name)
                .accessibilityValue(selected ? "Applied +3" : "Not applied")
            }
        }
    }

    private var secretToggle: some View {
        Button { secrets.toggle() } label: {
            Label("Secrets", systemImage: secrets ? "checkmark" : "xmark")
                .font(.subheadline.weight(.medium))
                .padding(.horizontal, 12).padding(.vertical, 9)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(secrets ? Color.accentColor.opacity(0.2) : .clear).interactive(), in: .capsule)
        .disabled((map?.document.secretCount ?? 0) == 0)
        .accessibilityAddTraits(secrets ? [.isSelected] : [])
    }

    private var toolbar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                if !branches.isEmpty {
                    branchButton("Main", branch: 0)
                    ForEach(branches) { area in
                        branchButton(area.kind == "imp_vault" ? "Vault" : "Mine", branch: area.branch)
                            .accessibilityLabel(area.label)
                    }
                }
                Spacer(minLength: 8)
                secretToggle
            }
            .padding(.horizontal, 10).padding(.vertical, 8)
        }
        .defaultScrollAnchor(.trailing, for: .alignment)
    }

    private func branchButton(_ label: String, branch value: Int) -> some View {
        Button { branchSelection = (profile, value) } label: {
            Text(label).font(.subheadline.weight(.medium))
                .padding(.horizontal, 14).padding(.vertical, 9)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(branch == value ? Color.accentColor.opacity(0.2) : .clear).interactive(), in: .capsule)
        .accessibilityAddTraits(branch == value ? [.isSelected] : [])
    }

    private var stage: some View {
        ZStack {
            Color.black
            NativeLevelMap(bundle: map, request: request, secrets: secrets,
                           animated: animated && scenePhase == .active, reduceMotion: reduceMotion,
                           expanded: expanded, label: title, navigate: navigate)
            if let error {
                VStack(spacing: 12) {
                    Text("Couldn’t load this map.").font(.headline)
                    Text(error).font(.caption).foregroundStyle(.white.opacity(0.7))
                        .multilineTextAlignment(.center)
                    Button("Try again") { retry += 1 }.buttonStyle(.glass)
                }.foregroundStyle(.white).padding(24)
            } else if map == nil {
                VStack(spacing: 12) {
                    ProgressView().tint(.white)
                    Text("Charting \(branch == 0 ? "floor \(depth)" : "the quest level")…")
                        .font(.subheadline).foregroundStyle(.white.opacity(0.7))
                }
            }
        }
        .clipped()
        .overlay(alignment: .topTrailing) {
            if let onExpand {
                Button("Expand map") { onExpand(branch, secrets) }
                    .buttonStyle(.glass).padding(8)
            }
        }
        .overlay(alignment: .bottom) {
            if expanded {
                HStack(spacing: 0) {
                    Button { navigate(-1) } label: {
                        Image(systemName: "chevron.left").frame(width: 52, height: 48)
                    }.disabled(floorIndex == 0).accessibilityLabel("Previous floor")
                    Divider().frame(height: 24)
                    Button { navigate(1) } label: {
                        Image(systemName: "chevron.right").frame(width: 52, height: 48)
                    }.disabled(floorIndex >= floors.count - 1).accessibilityLabel("Next floor")
                }
                .font(.headline).buttonStyle(.plain)
                .glassEffect(.regular.interactive(), in: .capsule)
                .padding(12)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
    }
}

private struct NativeLevelMap: UIViewRepresentable {
    let bundle: LevelMapBundle?
    let request: LevelMapRequest
    let secrets: Bool
    let animated: Bool
    let reduceMotion: Bool
    let expanded: Bool
    let label: String
    let navigate: (Int) -> Void

    func makeUIView(context: Context) -> MapViewport { MapViewport() }
    func updateUIView(_ view: MapViewport, context: Context) {
        view.update(bundle: bundle, request: request, secrets: secrets,
                    animated: animated, reduceMotion: reduceMotion, expanded: expanded,
                    label: label, navigate: navigate)
    }
    static func dismantleUIView(_ view: MapViewport, coordinator: ()) { view.stopAnimation() }
}

/// UIKit owns pinch, double-tap and pan recognition. At fit zoom an inline map
/// allows the Scout to scroll; the expanded map uses horizontal swipes for floors.
@MainActor final class MapViewport: UIView, UIGestureRecognizerDelegate {
    private(set) var request: LevelMapRequest?
    private var renderer: MapRenderer?
    private var clock = LevelMapClock(now: 0)
    private var secrets = false
    private var animated = false
    private var reduceMotion = false
    private var expanded = false
    private var navigate: (Int) -> Void = { _ in }
    private var displayLink: CADisplayLink?
    private var hadMultipleTouches = false
    private(set) var zoom: CGFloat = 1
    private(set) var pan = CGPoint.zero
    private lazy var linkTarget = MapFrameTarget(view: self)
    private lazy var panGesture = UIPanGestureRecognizer(target: self, action: #selector(drag(_:)))

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isOpaque = true
        clipsToBounds = true
        contentMode = .redraw
        isAccessibilityElement = true
        accessibilityTraits = [.image, .adjustable]
        let pinch = UIPinchGestureRecognizer(target: self, action: #selector(pinch(_:)))
        pinch.delegate = self
        addGestureRecognizer(pinch)
        panGesture.delegate = self
        addGestureRecognizer(panGesture)
        let doubleTap = UITapGestureRecognizer(target: self, action: #selector(doubleTap(_:)))
        doubleTap.numberOfTapsRequired = 2
        addGestureRecognizer(doubleTap)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func update(bundle: LevelMapBundle?, request: LevelMapRequest, secrets: Bool,
                animated: Bool, reduceMotion: Bool, expanded: Bool,
                label: String, navigate: @escaping (Int) -> Void) {
        let changed = self.request != request
        if self.request?.hasSameLocation(as: request) != true { zoom = 1; pan = .zero }
        self.request = request
        if let bundle {
            if renderer == nil || changed {
                renderer = MapRenderer(bundle)
                clock = LevelMapClock(now: ProcessInfo.processInfo.systemUptime * 1000)
            }
        } else { renderer = nil }
        self.secrets = secrets
        self.animated = animated
        self.reduceMotion = reduceMotion
        self.expanded = expanded
        self.navigate = navigate
        accessibilityLabel = label
        accessibilityCustomActions = expanded ? [
            UIAccessibilityCustomAction(name: "Previous floor", target: self, selector: #selector(previousFloor)),
            UIAccessibilityCustomAction(name: "Next floor", target: self, selector: #selector(nextFloor)),
        ] : nil
        updateAnimation()
        setNeedsDisplay()
    }

    override func didMoveToWindow() { super.didMoveToWindow(); updateAnimation() }
    override func layoutSubviews() { super.layoutSubviews(); constrain(); setNeedsDisplay() }

    private func updateAnimation() {
        guard window != nil, animated, !reduceMotion, renderer != nil else { stopAnimation(); return }
        guard displayLink == nil else { return }
        let link = CADisplayLink(target: linkTarget, selector: #selector(MapFrameTarget.frame))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 15, maximum: 30, preferred: 30)
        link.add(to: .main, forMode: .common)
        displayLink = link
    }
    func stopAnimation() { displayLink?.invalidate(); displayLink = nil }
    fileprivate func animationFrame() {
        guard !isHidden, let window, !convert(bounds, to: window).intersection(window.bounds).isEmpty else { return }
        setNeedsDisplay()
    }
    private var fitScale: CGFloat {
        guard let renderer else { return 1 }
        let fit = min(bounds.width / CGFloat(renderer.map.pixelWidth), bounds.height / CGFloat(renderer.map.pixelHeight))
        return max(0.01, fit >= 1 ? floor(fit) : fit)
    }
    private func constrain() {
        guard let renderer else { return }
        let x = max(0, (CGFloat(renderer.map.pixelWidth) * fitScale * zoom - bounds.width) / 2)
        let y = max(0, (CGFloat(renderer.map.pixelHeight) * fitScale * zoom - bounds.height) / 2)
        pan.x = min(x, max(-x, pan.x))
        pan.y = min(y, max(-y, pan.y))
    }
    private func zoom(to value: CGFloat, at anchor: CGPoint = .zero) {
        let next = min(8, max(1, value)), ratio = next / zoom
        pan = CGPoint(x: anchor.x - (anchor.x - pan.x) * ratio,
                      y: anchor.y - (anchor.y - pan.y) * ratio)
        zoom = next
        constrain()
        setNeedsDisplay()
    }
    @objc private func pinch(_ gesture: UIPinchGestureRecognizer) {
        hadMultipleTouches = true
        let point = gesture.location(in: self)
        zoom(to: zoom * gesture.scale, at: CGPoint(x: point.x - bounds.midX, y: point.y - bounds.midY))
        gesture.scale = 1
    }
    @objc private func doubleTap(_ gesture: UITapGestureRecognizer) {
        let point = gesture.location(in: self)
        zoom(to: zoom > 1 ? 1 : 2.5, at: CGPoint(x: point.x - bounds.midX, y: point.y - bounds.midY))
    }
    @objc private func drag(_ gesture: UIPanGestureRecognizer) {
        if gesture.state == .began { hadMultipleTouches = gesture.numberOfTouches > 1 }
        if gesture.numberOfTouches > 1 { hadMultipleTouches = true }
        let translation = gesture.translation(in: self)
        if zoom > 1 {
            pan.x += translation.x
            pan.y += translation.y
            gesture.setTranslation(.zero, in: self)
            constrain()
            setNeedsDisplay()
        } else if gesture.state == .ended, expanded, !hadMultipleTouches,
                  abs(translation.x) > 60, abs(translation.x) > abs(translation.y) * 1.5 {
            navigate(translation.x < 0 ? 1 : -1)
        }
    }
    override func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        guard gestureRecognizer === panGesture else { return true }
        if zoom > 1 { return true }
        let velocity = panGesture.velocity(in: self)
        return expanded && abs(velocity.x) > abs(velocity.y)
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        gestureRecognizer.view === self && otherGestureRecognizer.view === self
    }
    override func accessibilityIncrement() { zoom(to: zoom * 1.5) }
    override func accessibilityDecrement() { zoom(to: zoom / 1.5) }
    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        guard expanded else { return false }
        switch direction {
        case .left: navigate(1)
        case .right: navigate(-1)
        default: return false
        }
        return true
    }
    @objc private func previousFloor() -> Bool { navigate(-1); return true }
    @objc private func nextFloor() -> Bool { navigate(1); return true }

    override func draw(_ rect: CGRect) {
        guard let context = UIGraphicsGetCurrentContext() else { return }
        context.setFillColor(UIColor.black.cgColor)
        context.fill(bounds)
        guard let renderer else { return }
        constrain()
        let scale = fitScale * zoom
        context.saveGState()
        context.translateBy(x: bounds.midX + pan.x - CGFloat(renderer.map.pixelWidth) * scale / 2,
                            y: bounds.midY + pan.y - CGFloat(renderer.map.pixelHeight) * scale / 2)
        context.scaleBy(x: scale, y: scale)
        context.interpolationQuality = .none
        context.setShouldAntialias(false)
        renderer.draw(in: context,
                      elapsed: clock.elapsed(at: ProcessInfo.processInfo.systemUptime * 1000, reducedMotion: reduceMotion),
                      secrets: secrets)
        context.restoreGState()
    }
}

@MainActor private final class MapFrameTarget: NSObject {
    weak var view: MapViewport?
    init(view: MapViewport) { self.view = view }
    @objc func frame() { view?.animationFrame() }
}
/// The engine supplies draw order, source rectangles and all animation films.
/// Only cropped/tinted PNGs and unchanged scenery frames are cached here.
@MainActor private final class MapRenderer {
    let map: LevelMapDocument
    private var textures: [String: CGImage] = [:]
    private var crops: [LevelMapDocument.Draw: CGImage] = [:]
    private var scenery: CGImage?
    private var sceneryContext: CGContext?
    private var cellSprites: [[(sprite: Int, additive: Bool)]] = []
    private var spriteCells: [Int: Set<Int>] = [:]
    private var glowingSprites: Set<Int> = []
    private var frameKey: [Int] = []
    private var showingSecrets: Bool?
    private var masks: [String: CGImage] = [:]

    init(_ bundle: LevelMapBundle) {
        map = bundle.document
        glowingSprites = Set(map.scene.sprites.indices.filter { index in
            map.scene.sprites[index].frames.contains { $0.contains { $0.glow != nil } }
        })
        for (id, png) in bundle.assets {
            if let source = CGImageSourceCreateWithData(png as CFData, nil),
               let image = CGImageSourceCreateImageAtIndex(source, 0, nil) { textures[id] = image }
        }
    }

    private var rect: CGRect { CGRect(x: 0, y: 0, width: map.pixelWidth, height: map.pixelHeight) }
    private func bitmap() -> CGContext? {
        guard let context = CGContext(data: nil, width: map.pixelWidth, height: map.pixelHeight,
                                      bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.translateBy(x: 0, y: CGFloat(map.pixelHeight)); context.scaleBy(x: 1, y: -1)
        context.interpolationQuality = .none; context.setShouldAntialias(false)
        return context
    }

    /// CGImage is bottom-up when drawn in our top-left coordinate system.
    private func image(_ image: CGImage, in target: CGRect, context: CGContext) {
        context.saveGState()
        context.translateBy(x: target.minX, y: target.maxY); context.scaleBy(x: 1, y: -1)
        context.draw(image, in: CGRect(origin: .zero, size: target.size))
        context.restoreGState()
    }

    private func texture(_ draw: LevelMapDocument.Draw) -> CGImage? {
        if let cached = crops[draw] { return cached }
        guard let source = draw.source, let atlas = textures[draw.asset ?? ""],
              let cropped = atlas.cropping(to: CGRect(x: source[0], y: source[1], width: source[2], height: source[3])) else { return nil }
        guard let tint = draw.tint else { crops[draw] = cropped; return cropped }
        let width = cropped.width, height = cropped.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let tinted: CGImage? = pixels.withUnsafeMutableBytes { buffer in
            guard let context = CGContext(data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8,
                                          bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
            context.draw(cropped, in: CGRect(x: 0, y: 0, width: width, height: height))
            let bytes = buffer.bindMemory(to: UInt8.self)
            for pixel in 0..<(width * height) {
                for channel in 0..<3 { bytes[pixel * 4 + channel] = UInt8(Double(bytes[pixel * 4 + channel]) * tint[channel] / 255) }
            }
            return context.makeImage()
        }
        crops[draw] = tinted
        return tinted
    }

    private func command(_ draw: LevelMapDocument.Draw, in context: CGContext, elapsed: Double,
                         target: CGRect? = nil, alpha: Double = 1) {
        let d = draw.destination
        let destination = target ?? CGRect(x: d[0], y: d[1], width: d[2], height: d[3])
        context.saveGState()
        if draw.kind == "fill", let rgba = draw.rgba {
            context.setFillColor(CGColor(red: rgba[0] / 255, green: rgba[1] / 255, blue: rgba[2] / 255, alpha: rgba[3] / 255 * alpha))
            context.fill(destination)
        } else if let texture = texture(draw) {
            context.setAlpha((draw.opacity ?? 255) / 255 * alpha)
            if let glow = draw.glow, glow.strength(at: elapsed) > 0 {
                // Isolate the sprite so source-atop preserves its silhouette
                // instead of tinting the floor behind transparent pixels.
                context.beginTransparencyLayer(auxiliaryInfo: nil)
                image(texture, in: destination, context: context)
                context.setBlendMode(.sourceAtop)
                context.setFillColor(CGColor(red: glow.color[0] / 255, green: glow.color[1] / 255,
                                            blue: glow.color[2] / 255, alpha: glow.strength(at: elapsed)))
                context.fill(destination)
                context.endTransparencyLayer()
            } else { image(texture, in: destination, context: context) }
        }
        context.restoreGState()
    }

    private func layer(_ layer: LevelMapDocument.Layer, in context: CGContext, elapsed: Double) {
        context.saveGState(); context.setBlendMode(layer.blend == "add" ? .plusLighter : .normal)
        for (cell, sprite) in layer.cells.enumerated() {
            guard let sprite else { continue }
            context.saveGState()
            context.translateBy(x: CGFloat(cell % map.width * 16), y: CGFloat(cell / map.width * 16))
            let film = map.scene.sprites[sprite]
            for draw in film.frames[film.frameIndex(at: elapsed)] { command(draw, in: context, elapsed: elapsed) }
            context.restoreGState()
        }
        context.restoreGState()
    }

    func draw(in context: CGContext, elapsed: Double, secrets: Bool) {
        let layers = secrets ? map.scene.layers : map.scene.concealedLayers
        let emitters = secrets ? map.scene.emitters : map.scene.concealedEmitters
        let key = map.scene.sprites.map { $0.frameIndex(at: elapsed) }
        var dirty = Set<Int>()
        if showingSecrets != secrets || sceneryContext == nil {
            sceneryContext = bitmap()
            cellSprites = Array(repeating: [], count: map.width * map.height)
            spriteCells.removeAll()
            for entry in layers {
                for (cell, sprite) in entry.cells.enumerated() {
                    guard let sprite else { continue }
                    cellSprites[cell].append((sprite, entry.blend == "add"))
                    spriteCells[sprite, default: []].insert(cell)
                }
            }
            dirty = Set(cellSprites.indices)
        } else {
            for index in key.indices where key[index] != frameKey[index] || glowingSprites.contains(index) {
                dirty.formUnion(spriteCells[index] ?? [])
            }
        }
        if !dirty.isEmpty, let target = sceneryContext {
            // Version 3 commands stay within a tile. Redrawing only affected
            // tiles preserves layer order while letting glows advance at the
            // display rate independently of water/actor animation frames.
            for cell in dirty {
                target.saveGState()
                target.translateBy(x: CGFloat(cell % map.width * 16), y: CGFloat(cell / map.width * 16))
                target.setBlendMode(.copy); target.setFillColor(UIColor.black.cgColor)
                target.fill(CGRect(x: 0, y: 0, width: 16, height: 16))
                for entry in cellSprites[cell] {
                    target.setBlendMode(entry.additive ? .plusLighter : .normal)
                    for draw in map.scene.sprites[entry.sprite].frames[key[entry.sprite]] {
                        command(draw, in: target, elapsed: elapsed)
                    }
                }
                target.restoreGState()
            }
            scenery = target.makeImage()
        }
        frameKey = key
        if showingSecrets != secrets {
            masks.removeAll()
            for (name, names) in [("walls", ["raised", "walls", "room_walls", "boss_walls"]), ("darkness", ["darkness"])] {
                if let target = bitmap() {
                    for entry in layers where names.contains(entry.name) { layer(entry, in: target, elapsed: 0) }
                    masks[name] = target.makeImage()
                }
            }
            showingSecrets = secrets
        }
        guard let scenery else { return }
        image(scenery, in: rect, context: context)
        guard !emitters.isEmpty else { return }
        // Start with scenery as the additive destination. Removing occlusion
        // silhouettes reveals the untouched scenery underneath this overlay.
        context.saveGState(); context.beginTransparencyLayer(auxiliaryInfo: nil)
        image(scenery, in: rect, context: context)
        let chasms = CGMutablePath()
        for emitter in emitters where emitter.clipToChasm == true {
            chasms.addRect(CGRect(x: emitter.cell % map.width * 16, y: emitter.cell / map.width * 16, width: 16, height: 16))
        }
        context.saveGState(); context.addPath(chasms); context.clip()
        for emitter in emitters where emitter.clipToChasm == true { particles(emitter, in: context, elapsed: elapsed) }
        context.restoreGState()
        for emitter in emitters where emitter.wallMask == true && emitter.clipToChasm != true { particles(emitter, in: context, elapsed: elapsed) }
        erase("walls", in: context)
        for emitter in emitters where emitter.wallMask != true && emitter.clipToChasm != true { particles(emitter, in: context, elapsed: elapsed) }
        erase("darkness", in: context)
        context.endTransparencyLayer(); context.restoreGState()
    }

    private func erase(_ name: String, in context: CGContext) {
        guard let mask = masks[name] else { return }
        context.saveGState(); context.setBlendMode(.destinationOut)
        image(mask, in: rect, context: context); context.restoreGState()
    }
    private func particles(_ emitter: LevelMapDocument.Emitter, in context: CGContext, elapsed: Double) {
        context.saveGState(); context.setBlendMode(emitter.blend == "add" ? .plusLighter : .normal)
        context.translateBy(x: CGFloat(emitter.cell % map.width * 16), y: CGFloat(emitter.cell / map.width * 16))
        for particle in emitter.particles {
            guard let state = emitter.state(of: particle, at: elapsed), state.scale > 0, state.scaleX > 0, state.scaleY > 0, state.alpha > 0 else { continue }
            context.saveGState(); context.translateBy(x: state.x, y: state.y)
            context.rotate(by: state.angle); context.scaleBy(x: state.scale * state.scaleX, y: state.scale * state.scaleY)
            let size = emitter.image.destination
            command(emitter.image, in: context, elapsed: elapsed,
                    target: CGRect(x: -size[2] / 2, y: -size[3] / 2, width: size[2], height: size[3]), alpha: state.alpha)
            context.restoreGState()
        }
        context.restoreGState()
    }
}
