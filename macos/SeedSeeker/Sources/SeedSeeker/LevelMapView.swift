import AppKit
import ImageIO
import SeedSeekerKit
import SwiftUI

/// The inline map stays attached to its floor; the sheet owns a separate map
/// and navigation state. Only the selected trinket belongs to the shared Scout.
struct LevelMapView: View {
    let world: ScoutWorld
    let depth: Int
    let floors: [Int]
    let challenges: Int
    let active: Bool
    let changingTrinket: Bool
    let onSelectTrinket: (String) -> Void
    @State private var expandedMap: ExpandedMapSelection?

    private struct ExpandedMapSelection: Identifiable {
        let id = UUID()
        let branch: Int
        let secrets: Bool
    }

    var body: some View {
        LevelMapPanel(world: world, depth: depth, challenges: challenges,
                      animated: active && expandedMap == nil, mapHeight: 260, onExpand: { branch, secrets in
                          expandedMap = ExpandedMapSelection(branch: branch, secrets: secrets)
                      })
            .padding(.vertical, 8)
            .sheet(item: $expandedMap) { selection in
                ExpandedLevelMapView(world: world, initialDepth: depth, floors: floors,
                                     initialBranch: selection.branch, initialSecrets: selection.secrets,
                                     challenges: challenges, changingTrinket: changingTrinket,
                                     onSelectTrinket: onSelectTrinket)
            }
    }
}

private struct ExpandedLevelMapView: View {
    let world: ScoutWorld
    let initialDepth: Int
    let floors: [Int]
    let initialBranch: Int
    let initialSecrets: Bool
    let challenges: Int
    let changingTrinket: Bool
    let onSelectTrinket: (String) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var selectedDepth: Int?

    private var currentDepth: Int { selectedDepth ?? initialDepth }

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 10) {
                HStack(spacing: 4) {
                    Button { navigate(-1) } label: { floorArrow("chevron.left") }
                        .buttonStyle(.plain)
                        .help("Previous floor (K)").keyboardShortcut("k", modifiers: [])
                        .accessibilityLabel("Previous floor")
                        .disabled(floors.first == currentDepth)
                    Picker("Floor", selection: Binding(get: { currentDepth }, set: { selectedDepth = $0 })) {
                        ForEach(floors, id: \.self) { Text("Floor \($0)").tag($0) }
                    }.labelsHidden().fixedSize()
                    Button { navigate(1) } label: { floorArrow("chevron.right") }
                        .buttonStyle(.plain)
                        .help("Next floor (J)").keyboardShortcut("j", modifiers: [])
                        .accessibilityLabel("Next floor")
                        .disabled(floors.last == currentDepth)
                }
                if let feeling = world.feelings[currentDepth] { FloorFeelingSpriteView(feeling: feeling) }
                if let quest = world.quests.first(where: { $0.depth == currentDepth }) {
                    Text(quest.variant.label).font(.caption).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
                HStack(spacing: 4) {
                    ForEach(Array(world.trinketOrder.prefix(4))) { item in
                        Button { onSelectTrinket(item.id) } label: {
                            ItemSpriteView(item: item, pointSize: 22).padding(5)
                                .background(world.selectedTrinket == item.id ? Color.shatteredMint.opacity(0.15) : .clear,
                                            in: RoundedRectangle(cornerRadius: 6))
                                .overlay(RoundedRectangle(cornerRadius: 6)
                                    .strokeBorder(world.selectedTrinket == item.id ? Color.shatteredMint : Color.secondary.opacity(0.25)))
                        }
                        .buttonStyle(.plain).disabled(changingTrinket).help(item.name)
                        .accessibilityLabel(item.name)
                        .accessibilityValue(world.selectedTrinket == item.id ? "Applied at +3" : "Not selected")
                    }
                }
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            LevelMapPanel(world: world, depth: currentDepth, challenges: challenges, animated: true,
                          initialBranch: initialBranch, initialSecrets: initialSecrets)
        }
        .padding(16).frame(minWidth: 780, idealWidth: 980, minHeight: 600, idealHeight: 760)
    }

    private func floorArrow(_ systemName: String) -> some View {
        Image(systemName: systemName)
            .font(.system(size: 11, weight: .medium))
            .frame(width: 24, height: 24)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
    }

    private func navigate(_ delta: Int) {
        guard let index = floors.firstIndex(of: currentDepth), floors.indices.contains(index + delta) else { return }
        selectedDepth = floors[index + delta]
    }
}

/// Loading follows the disclosed map's request, independently of visibility.
/// Visibility only pauses animation, so stale scroll geometry cannot strand a
/// newly opened inline map on its loading indicator.
struct LevelMapPanel: View {
    let world: ScoutWorld
    let depth: Int
    let challenges: Int
    let animated: Bool
    var mapHeight: CGFloat?
    var onExpand: ((Int, Bool) -> Void)?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var branchSelection: (profile: LevelMapRequest, branch: Int)?
    @State private var secrets = false
    @State private var bundle: LevelMapBundle?
    @State private var loadedKey: LevelMapRequest?
    @State private var parentKey: LevelMapRequest?
    @State private var parentBranches: [LevelMapDocument.Branch] = []
    @State private var error: String?
    @State private var retry = 0

    init(world: ScoutWorld, depth: Int, challenges: Int, animated: Bool,
         mapHeight: CGFloat? = nil, onExpand: ((Int, Bool) -> Void)? = nil,
         initialBranch: Int = 0, initialSecrets: Bool = false) {
        self.world = world; self.depth = depth; self.challenges = challenges
        self.animated = animated; self.mapHeight = mapHeight; self.onExpand = onExpand
        let profile = LevelMapRequest(seed: world.seed, depth: depth, challenges: challenges,
                                      selectedTrinket: world.selectedTrinket)
        _branchSelection = State(initialValue: (profile, initialBranch))
        _secrets = State(initialValue: initialSecrets)
    }

    private var profile: LevelMapRequest {
        LevelMapRequest(seed: world.seed, depth: depth, challenges: challenges,
                        selectedTrinket: world.selectedTrinket)
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
    private var branches: [LevelMapDocument.Branch] { parentKey?.hasSameLocation(as: profile) == true ? parentBranches : [] }
    private var title: String {
        branch == 0 ? "Floor \(depth) layout" : (map?.document.kind == "imp_vault" ? "Imp Vault" : "Blacksmith Mine")
    }
    private struct LoadKey: Hashable { let request: LevelMapRequest; let retry: Int }

    var body: some View {
        VStack(spacing: 6) {
            toolbar
            stage.frame(height: mapHeight)
                .overlay(alignment: .topTrailing) {
                    if let onExpand {
                        Button { onExpand(branch, secrets) } label: { Label("Expand", systemImage: "arrow.up.left.and.arrow.down.right") }
                            .controlSize(.small).padding(8)
                    }
                }
        }
        .task(id: LoadKey(request: request, retry: retry)) {
            let requested = request, parent = profile
            error = nil
            do {
                // Refresh branch availability with the trinket, while keeping
                // the selected area when it still exists in the new scene.
                if parentKey != parent {
                    let main = try await LevelMapClient.shared.load(parent)
                    guard !Task.isCancelled, requested == request else { return }
                    parentKey = parent; parentBranches = main.document.branches
                    if requested.branch == 0 {
                        bundle = main; loadedKey = requested
                        return
                    }
                    if !parentBranches.contains(where: { $0.branch == requested.branch }) {
                        branchSelection = nil
                        return
                    }
                }
                let next = try await LevelMapClient.shared.load(requested)
                guard !Task.isCancelled, requested == request else { return }
                bundle = next; loadedKey = requested
            } catch {
                guard !Task.isCancelled, requested == request else { return }
                self.error = error.localizedDescription
            }
        }
    }

    private var toolbar: some View {
        HStack {
            if !branches.isEmpty {
                Picker("Level area", selection: Binding(get: { branch }, set: { branchSelection = (profile, $0) })) {
                    Text("Main").tag(0)
                    ForEach(branches) { Text($0.label).tag($0.branch) }
                }.pickerStyle(.segmented).labelsHidden().fixedSize()
            }
            Spacer(minLength: 0)
            Toggle("Secrets", isOn: $secrets).toggleStyle(.button)
                .disabled(map?.document.secretCount == 0)
                .help(map?.document.secretCount == 0 ? "No secrets on this map" : "Reveal secret rooms, doors and traps")
        }
    }

    private var stage: some View {
        ZStack {
            Color.black
            // Keep the AppKit viewport mounted while loading. Replacing it
            // with the spinner would discard zoom and pan on trinket changes.
            TimelineView(.animation(paused: !animated || reduceMotion || map == nil)) { _ in
                NativeLevelMap(bundle: map, request: request, secrets: secrets,
                               time: ProcessInfo.processInfo.systemUptime * 1000, reduceMotion: reduceMotion,
                               label: title)
            }
            if let error {
                VStack(spacing: 10) {
                    Text("Couldn’t load this map.").font(.headline)
                    Text(error).font(.caption).multilineTextAlignment(.center)
                    Button("Try Again") { retry += 1 }
                }.foregroundStyle(.white).padding()
            } else if map == nil {
                VStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text("Charting \(branch == 0 ? "floor \(depth)" : "the quest level")…").font(.caption)
                }.foregroundStyle(.white)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .accessibilityElement(children: .contain).accessibilityLabel(title)
    }
}

private struct NativeLevelMap: NSViewRepresentable {
    let bundle: LevelMapBundle?
    let request: LevelMapRequest
    let secrets: Bool
    let time: Double
    let reduceMotion: Bool
    let label: String

    func makeNSView(context: Context) -> MapViewport { MapViewport() }
    func updateNSView(_ view: MapViewport, context: Context) {
        view.update(bundle: bundle, request: request, time: time)
        view.secrets = secrets
        view.elapsed = view.clock.elapsed(at: time, reducedMotion: reduceMotion)
        view.setAccessibilityLabel(label)
        view.needsDisplay = true
    }
}

/// AppKit consumes wheel, magnification and dragging inside this viewport. It
/// keeps the surrounding Scout scroll position and supports keyboard controls
/// without drawing a focus border over the map.
@MainActor final class MapViewport: NSView {
    private(set) var request: LevelMapRequest?
    private var renderer: MapRenderer?
    var hasMap: Bool { renderer != nil }
    var secrets = false
    var elapsed: Double = 0
    var clock = LevelMapClock(now: 0)
    private(set) var zoom: CGFloat = 1
    private(set) var pan = CGPoint.zero
    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        focusRingType = .none
        toolTip = "Scroll or pinch to zoom. Drag to pan. Press 0 to fit the map."
        setAccessibilityElement(true)
        setAccessibilityRole(.image)
        setAccessibilityHelp("Scroll or pinch to zoom. Drag or use arrow keys to pan. Press zero to fit the map.")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    func update(bundle: LevelMapBundle?, request: LevelMapRequest, time: Double) {
        let sceneChanged = self.request != request
        if self.request?.hasSameLocation(as: request) != true {
            zoom = 1; pan = .zero
        }
        self.request = request
        if let bundle {
            if renderer == nil || sceneChanged {
                renderer = MapRenderer(bundle)
                clock = LevelMapClock(now: time)
            }
        } else {
            renderer = nil
        }
    }

    private var fitScale: CGFloat {
        guard let renderer else { return 1 }
        return max(0.01, min((bounds.width - 16) / CGFloat(renderer.map.pixelWidth),
                             (bounds.height - 16) / CGFloat(renderer.map.pixelHeight)))
    }
    private func constrain() {
        guard let renderer else { return }
        let x = max(0, (CGFloat(renderer.map.pixelWidth) * fitScale * zoom - bounds.width) / 2)
        let y = max(0, (CGFloat(renderer.map.pixelHeight) * fitScale * zoom - bounds.height) / 2)
        pan.x = min(x, max(-x, pan.x)); pan.y = min(y, max(-y, pan.y))
    }
    private func zoom(to value: CGFloat, at anchor: CGPoint = .zero) {
        let next = min(8, max(1, value)), ratio = next / zoom
        pan = CGPoint(x: anchor.x - (anchor.x - pan.x) * ratio,
                      y: anchor.y - (anchor.y - pan.y) * ratio)
        zoom = next; constrain(); needsDisplay = true
    }
    private func anchor(_ event: NSEvent) -> CGPoint {
        let point = convert(event.locationInWindow, from: nil)
        return CGPoint(x: point.x - bounds.midX, y: point.y - bounds.midY)
    }
    override func scrollWheel(with event: NSEvent) {
        zoom(to: zoom * exp(-event.scrollingDeltaY * (event.hasPreciseScrollingDeltas ? 0.008 : 0.08)), at: anchor(event))
    }
    override func magnify(with event: NSEvent) { zoom(to: zoom * (1 + event.magnification), at: anchor(event)) }
    override func mouseDown(with event: NSEvent) { window?.makeFirstResponder(self) }
    override func mouseDragged(with event: NSEvent) {
        pan.x += event.deltaX; pan.y += event.deltaY; constrain(); needsDisplay = true
    }
    override func keyDown(with event: NSEvent) {
        guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty else {
            super.keyDown(with: event); return
        }
        switch event.charactersIgnoringModifiers {
        case "+", "=": zoom(to: zoom * 1.5)
        case "-": zoom(to: zoom / 1.5)
        case "0": zoom = 1; pan = .zero
        case String(UnicodeScalar(NSLeftArrowFunctionKey)!): pan.x += 24
        case String(UnicodeScalar(NSRightArrowFunctionKey)!): pan.x -= 24
        case String(UnicodeScalar(NSUpArrowFunctionKey)!): pan.y += 24
        case String(UnicodeScalar(NSDownArrowFunctionKey)!): pan.y -= 24
        default: super.keyDown(with: event); return
        }
        constrain(); needsDisplay = true
    }
    override func draw(_ dirtyRect: NSRect) {
        guard !visibleRect.isEmpty, let context = NSGraphicsContext.current?.cgContext else { return }
        context.setFillColor(NSColor.black.cgColor); context.fill(bounds)
        guard let renderer else { return }
        constrain()
        let scale = fitScale * zoom
        context.saveGState()
        context.translateBy(x: bounds.midX + pan.x - CGFloat(renderer.map.pixelWidth) * scale / 2,
                            y: bounds.midY + pan.y - CGFloat(renderer.map.pixelHeight) * scale / 2)
        context.scaleBy(x: scale, y: scale)
        context.interpolationQuality = .none
        renderer.draw(in: context, elapsed: elapsed, secrets: secrets)
        context.restoreGState()
    }
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
                target.setBlendMode(.copy); target.setFillColor(NSColor.black.cgColor)
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
