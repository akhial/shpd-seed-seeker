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
    @Namespace private var mapZoom

    var body: some View {
        LevelMapPanel(world: world, depth: depth, challenges: challenges,
                      animated: active && expandedMap == nil, mapHeight: 280,
                      onExpand: { branch, secrets in
                          expandedMap = ExpandedMapSelection(world: world, floors: floors,
                                                             changingTrinket: changingTrinket,
                                                             branch: branch, secrets: secrets)
                      })
        // The inline map grows into the full-screen map and shrinks back.
        .matchedTransitionSource(id: "map", in: mapZoom)
        .fullScreenCover(item: $expandedMap) { selection in
            ExpandedLevelMapView(selection: selection, initialDepth: depth,
                                 challenges: challenges,
                                 onSelectTrinket: onSelectTrinket)
                .navigationTransition(.zoom(sourceID: "map", in: mapZoom))
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
            // Only the unsafe edges show through: the status bar keeps the
            // header's colour and the home indicator sits on the dungeon's black.
            .background {
                VStack(spacing: 0) { AppTheme.background; Color.black }.ignoresSafeArea()
            }
            // A full-screen cover starts a new presentation, outside the app's tint.
            .tint(AppTheme.accent)
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
    @State private var toolbarWidth: CGFloat = 0
    @Namespace private var branchLens

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
                .zIndex(1)
                if !branches.isEmpty { toolbar.zIndex(1) }
            } else {
                toolbar.zIndex(1)
            }
            // Controls draw above the map so pressed glass can stretch over it.
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
        TrinketGlassShortcuts(world: world, enabled: !changingTrinket, size: 38, onSelect: onSelectTrinket)
    }

    /// Secrets are an eye that opens, not a checkbox: the symbol morphs and
    /// the glass takes on the accent while hidden rooms are revealed.
    private var secretToggle: some View {
        let available = (map?.document.secretCount ?? 0) > 0
        return Button {
            withAnimation(reduceMotion ? nil : .snappy) { secrets.toggle() }
        } label: {
            Label("Secrets", systemImage: secrets ? "eye" : "eye.slash")
                .contentTransition(.symbolEffect(.replace))
                .font(.subheadline.weight(.medium))
                .foregroundStyle(secrets ? AppTheme.upgrade : Color.primary.opacity(available ? 0.85 : 0.4))
                .padding(.horizontal, 14).padding(.vertical, 9)
                .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .glassEffect(.regular.tint(secrets ? AppTheme.accent.opacity(0.24) : nil).interactive(), in: .capsule)
        .disabled(!available)
        .sensoryFeedback(.selection, trigger: secrets)
        .accessibilityAddTraits(secrets ? [.isSelected] : [])
    }

    private var toolbar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            GlassEffectContainer(spacing: 8) {
                HStack(spacing: 8) {
                    Spacer(minLength: 0)
                    if !branches.isEmpty {
                        branchButton("Main", branch: 0)
                        ForEach(branches) { area in
                            branchButton(area.kind == "imp_vault" ? "Vault" : "Mine", branch: area.branch)
                                .accessibilityLabel(area.label)
                        }
                    }
                    secretToggle
                }
                .padding(.horizontal, 10).padding(.vertical, 8)
                .frame(minWidth: toolbarWidth)
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { toolbarWidth = $0 }
        .scrollClipDisabled()
        .defaultScrollAnchor(.trailing, for: .alignment)
    }

    private func branchButton(_ label: String, branch value: Int) -> some View {
        Button {
            withAnimation(AppTheme.glassSpring(reduceMotion)) { branchSelection = (profile, value) }
        } label: {
            Text(label).font(.subheadline.weight(.medium))
                .padding(.horizontal, 14).padding(.vertical, 9)
                .contentShape(.capsule)
                .glassChoice("branch-\(value)", selected: branch == value, lens: "branch", in: branchLens)
        }
        .buttonStyle(.plain)
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
                Button("Expand map", systemImage: "arrow.up.left.and.arrow.down.right") { onExpand(branch, secrets) }
                    .labelStyle(.iconOnly)
                    .font(.subheadline.weight(.semibold))
                    .buttonStyle(.glass(.clear))
                    .buttonBorderShape(.circle)
                    .controlSize(.large)
                    .padding(10)
            }
        }
        .overlay(alignment: .bottom) {
            if expanded {
                // Clear glass lets the dungeon refract through the stepper.
                HStack(spacing: 0) {
                    Button { navigate(-1) } label: {
                        Image(systemName: "chevron.left").frame(width: 52, height: 48).contentShape(.rect)
                    }
                    .disabled(floorIndex == 0).opacity(floorIndex == 0 ? 0.3 : 1)
                    .accessibilityLabel("Previous floor")
                    Text("Floor \(depth)")
                        .font(.subheadline.weight(.semibold)).monospacedDigit()
                        .contentTransition(.numericText(value: Double(depth)))
                        .frame(minWidth: 70)
                        .accessibilityHidden(true)
                    Button { navigate(1) } label: {
                        Image(systemName: "chevron.right").frame(width: 52, height: 48).contentShape(.rect)
                    }
                    .disabled(floorIndex >= floors.count - 1).opacity(floorIndex >= floors.count - 1 ? 0.3 : 1)
                    .accessibilityLabel("Next floor")
                }
                .font(.headline).buttonStyle(.plain)
                .foregroundStyle(.white)
                .glassEffect(.clear.interactive(), in: .capsule)
                .animation(reduceMotion ? nil : .snappy, value: depth)
                .sensoryFeedback(.selection, trigger: depth)
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
    private(set) var inspectedCell: Int?
    private var itemOverlay: MapItemOverlayHost?
    private let ancestorScrollGestures = NSHashTable<UIPanGestureRecognizer>.weakObjects()
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
        doubleTap.delegate = self
        addGestureRecognizer(doubleTap)
        let tap = UITapGestureRecognizer(target: self, action: #selector(tap(_:)))
        tap.delegate = self
        tap.require(toFail: doubleTap)
        addGestureRecognizer(tap)
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(hover(_:)))
        addGestureRecognizer(hover)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func update(bundle: LevelMapBundle?, request: LevelMapRequest, secrets: Bool,
                animated: Bool, reduceMotion: Bool, expanded: Bool,
                label: String, navigate: @escaping (Int) -> Void) {
        let changed = self.request != request
        if changed || bundle == nil || self.secrets != secrets { dismissInspection(animated: false) }
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
        var actions = expanded ? [
            UIAccessibilityCustomAction(name: "Previous floor", target: self, selector: #selector(previousFloor)),
            UIAccessibilityCustomAction(name: "Next floor", target: self, selector: #selector(nextFloor)),
        ] : []
        // VoiceOver can inspect the same visible items without locating a tiny
        // map sprite. The engine's item name is also the action's label.
        for tip in renderer?.map.itemTooltips ?? [] where secrets || !tip.hidden {
            actions.append(UIAccessibilityCustomAction(name: tip.items.map(\.name).joined(separator: ", ")) { [weak self] _ in
                guard let self, let anchor = self.accessibilityInspectionAnchor else { return false }
                self.presentInspection(tip, at: anchor)
                return self.inspectedCell == tip.cell
            })
        }
        accessibilityCustomActions = actions.isEmpty ? nil : actions
        updateAnimation()
        setNeedsDisplay()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        observeAncestorScrolling()
        if window == nil { dismissInspection(animated: false) }
        updateAnimation()
    }
    override func didMoveToSuperview() {
        super.didMoveToSuperview()
        observeAncestorScrolling()
    }

    private func observeAncestorScrolling() {
        for gesture in ancestorScrollGestures.allObjects {
            gesture.removeTarget(self, action: #selector(ancestorDidScroll(_:)))
        }
        ancestorScrollGestures.removeAllObjects()
        var ancestor = superview
        while let view = ancestor {
            if let scroll = view as? UIScrollView {
                scroll.panGestureRecognizer.addTarget(self, action: #selector(ancestorDidScroll(_:)))
                ancestorScrollGestures.add(scroll.panGestureRecognizer)
            }
            ancestor = view.superview
        }
    }

    @objc private func ancestorDidScroll(_ gesture: UIPanGestureRecognizer) {
        if gesture.state == .began || gesture.state == .changed { dismissInspection(animated: false) }
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        if let itemOverlay, itemOverlay.frame != bounds {
            dismissInspection(animated: false)
            itemOverlay.frame = bounds
        }
        constrain()
        setNeedsDisplay()
    }

    func dismissInspection(animated: Bool = true) {
        inspectedCell = nil
        itemOverlay?.presentation.show(nil, animated: animated && !reduceMotion)
        // Keep the empty container mounted while the glass dematerializes.
        // Its hit test immediately resumes passing map gestures through.
        isAccessibilityElement = true
    }

    func inspectItem(at point: CGPoint, toggleSelection: Bool = false) {
        guard let renderer else { dismissInspection(); return }
        if !toggleSelection,
           let frame = itemOverlay?.presentation.card?.frame,
           frame.insetBy(dx: -12, dy: -12).contains(point) { return }
        constrain()
        let scale = fitScale * zoom
        let tip = renderer.map.itemAt(
            x: (point.x - bounds.midX - pan.x) / scale + CGFloat(renderer.map.pixelWidth) / 2,
            y: (point.y - bounds.midY - pan.y) / scale + CGFloat(renderer.map.pixelHeight) / 2,
            secrets: secrets)
        guard let tip else { dismissInspection(); return }
        if tip.cell == inspectedCell {
            if toggleSelection { dismissInspection() }
            return
        }
        presentInspection(tip, at: point)
    }

    private func presentInspection(_ tip: LevelMapDocument.ItemTooltip, at point: CGPoint) {
        let visible = inspectionVisibleBounds(around: point)
        guard !visible.isNull, visible.width > 0, visible.height > 0 else { return }
        let margin = min(8, visible.width / 8, visible.height / 8)
        let available = visible.insetBy(dx: margin, dy: margin)
        let width = min(330, available.width)
        let measuring = UIHostingController(rootView: MapItemCard(tip: tip).frame(width: width))
        let measured = measuring.sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude))
        let height = min(measured.height, available.height)
        let x = max(available.minX, min(point.x + 16, available.maxX - width))
        let below = point.y + 16
        let y = max(available.minY, min(below + height < available.maxY ? below : point.y - height - 16,
                                       available.maxY - height))
        if itemOverlay == nil {
            let overlay = MapItemOverlayHost(frame: bounds) { [weak self] in self?.dismissInspection() }
            addSubview(overlay)
            itemOverlay = overlay
            overlay.layoutIfNeeded()
        }
        inspectedCell = tip.cell
        isAccessibilityElement = false
        itemOverlay?.presentation.show(.init(tip: tip, frame: CGRect(x: x, y: y, width: width, height: height)),
                                       animated: !reduceMotion)
        if UIAccessibility.isVoiceOverRunning {
            UIAccessibility.post(notification: .layoutChanged, argument: itemOverlay)
        }
    }

    /// A map can be partly above the Scout's scroll viewport. Its own bounds
    /// still describe the full map, so using them would tuck the card's Close
    /// control behind the pinned floor heading or another clipping ancestor.
    private var clippedInspectionBounds: CGRect {
        var visible = bounds
        var ancestor = superview
        while let view = ancestor {
            if view.clipsToBounds {
                let area = (view as? UIScrollView).map { $0.bounds.inset(by: $0.adjustedContentInset) } ?? view.bounds
                visible = visible.intersection(convert(area, from: view))
            }
            ancestor = view.superview
        }
        if let window {
            visible = visible.intersection(convert(window.bounds.inset(by: window.safeAreaInsets), from: window))
        }
        return visible
    }

    private func inspectionPointIsUncovered(_ point: CGPoint) -> Bool {
        guard let window else { return true }
        guard let hit = window.hitTest(convert(point, to: window), with: nil) else { return false }
        return hit === self || hit.isDescendant(of: self)
    }

    /// VoiceOver chooses an item by name, so its placement anchor must come
    /// from the visible map rather than the possibly clipped map center.
    private var accessibilityInspectionAnchor: CGPoint? {
        let visible = clippedInspectionBounds
        guard !visible.isNull, !visible.isEmpty else { return nil }
        let center = CGPoint(x: visible.midX, y: visible.midY)
        if inspectionPointIsUncovered(center) { return center }
        var y = visible.minY + 1
        while y < visible.maxY {
            let candidate = CGPoint(x: visible.midX, y: y)
            if inspectionPointIsUncovered(candidate) {
                let band = inspectionVisibleBounds(around: candidate)
                return CGPoint(x: band.midX, y: band.midY)
            }
            y += 4
        }
        return nil
    }

    func inspectionVisibleBounds(around point: CGPoint) -> CGRect {
        let visible = clippedInspectionBounds
        guard !visible.isNull, visible.contains(point) else { return .null }
        guard window != nil else { return visible }

        // Pinned SwiftUI section headers are siblings, not clipping ancestors.
        // Hit testing the column of the tapped sprite finds the unobscured band
        // without depending on SwiftUI's private hosting view hierarchy.
        func uncovered(_ y: CGFloat) -> Bool {
            inspectionPointIsUncovered(CGPoint(x: point.x, y: y))
        }
        guard uncovered(point.y) else { return .null }
        var top = point.y, bottom = point.y
        while top > visible.minY {
            let next = max(visible.minY, top - 4)
            guard uncovered(next) else { break }
            top = next
        }
        while bottom < visible.maxY {
            let next = min(visible.maxY - 0.5, bottom + 4)
            guard next > bottom, uncovered(next) else { break }
            bottom = next
        }
        return CGRect(x: visible.minX, y: top, width: visible.width, height: bottom - top)
    }

    @objc private func tap(_ gesture: UITapGestureRecognizer) {
        inspectItem(at: gesture.location(in: self), toggleSelection: true)
    }
    @objc private func hover(_ gesture: UIHoverGestureRecognizer) {
        switch gesture.state {
        case .began, .changed: inspectItem(at: gesture.location(in: self))
        case .ended, .cancelled: dismissInspection()
        default: break
        }
    }

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
        dismissInspection(animated: false)
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
        dismissInspection(animated: false)
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
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        // A tooltip may scroll independently; touching its content must not pan,
        // zoom, or dismiss the map underneath it.
        itemOverlay?.presentation.card?.frame.contains(touch.location(in: self)) != true
    }
    override func accessibilityPerformEscape() -> Bool {
        guard inspectedCell != nil else { return false }
        dismissInspection()
        UIAccessibility.post(notification: .layoutChanged, argument: self)
        return true
    }
    override func accessibilityIncrement() { zoom(to: zoom * 1.5) }
    override func accessibilityDecrement() { zoom(to: zoom / 1.5) }
    override func accessibilityScroll(_ direction: UIAccessibilityScrollDirection) -> Bool {
        guard expanded else { return false }
        dismissInspection(animated: false)
        switch direction {
        case .left: navigate(1)
        case .right: navigate(-1)
        default: return false
        }
        return true
    }
    @objc private func previousFloor() -> Bool { dismissInspection(animated: false); navigate(-1); return true }
    @objc private func nextFloor() -> Bool { dismissInspection(animated: false); navigate(1); return true }

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
