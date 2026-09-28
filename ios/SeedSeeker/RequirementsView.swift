import SwiftUI
import SeedSeekerKit
import UIKit

/// The board draws what the shared core's requirement board says — the folded
/// entries, every chip's words, what each chip may join — and every gesture
/// is one of its edits, named by row key: keys, rather than positions,
/// survive every rewrite.
struct RequirementsView: View {
    @Binding var query: SavedQuery
    let interaction: RequirementBoardInteraction
    @AppStorage("compactChips") private var compactChips = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Namespace private var glass
    @GestureState private var gestureActive = false
    @State private var editor: RequirementsEditorPresentation?
    @State private var resinPresented = false
    @State private var blanketsExpanded = false
    @State private var blanketHelp = false
    @State private var chipFrames: [String: CGRect] = [:]
    @State private var boardFrame = CGRect.zero
    @State private var hoverKey: Int64?
    @State private var settling = false
    @State private var liftGeneration = UUID()
    @State private var suppressEditingUntil = Date.distantPast
    @State private var stackKey: RequirementsStackPresentation?
    @State private var resinSource = "resin"
    @State private var landedKey: Int64?
    /// Why the last drop could not join, shown in the hint's place for a moment.
    @State private var notice: String?
    @State private var noticeReset: Task<Void, Never>?
    @AppStorage("learnedGrouping") private var learnedGrouping = false
    @Namespace private var sheetZoom

    private var requirements: [ItemRequirement] { query.requirements }
    /// The shared core's board, memoized per change of the list, so drag
    /// frames and hover passes never ask the core again.
    private var snapshot: RequirementBoard { query.board }
    private var lift: RequirementLift? {
        get { interaction.lift }
        nonmutating set { interaction.lift = newValue }
    }

    var body: some View {
        GlassEffectContainer(spacing: 10) {
          VStack(alignment: .leading, spacing: 18) {
            board(blanket: false)
            if let notice {
                Label(notice, systemImage: "exclamationmark.triangle")
                    .font(.footnote)
                    .foregroundStyle(.orange)
                    .padding(.top, -8)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            } else if let boardHint {
                Label(boardHint.text, systemImage: boardHint.symbol)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .padding(.top, -8)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
            VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Button {
                    withAnimation(boardSpring) { blanketsExpanded.toggle() }
                } label: {
                    HStack(spacing: 9) {
                        Image(systemName: "square.3.layers.3d")
                            .font(.subheadline)
                        Text("Blanket Requirements (\(snapshot.blanketCount))")
                        Image(systemName: blanketsExpanded ? "chevron.up" : "chevron.down")
                            .font(.caption2.weight(.semibold))
                    }
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(Color.primary.opacity(blanketsExpanded ? 1 : 0.85))
                    .padding(.horizontal, 15)
                    .frame(minHeight: 46)
                    .glassEffect(.regular.interactive(), in: .capsule)
                    .glassEffectID("blanket-disclosure", in: glass)
                }
                .buttonStyle(.plain)
                .background(frameReader(id: "blanket-disclosure"))
                .accessibilityHint(blanketsExpanded ? "Collapse blankets" : "Expand blankets")
                Button { blanketHelp = true } label: {
                    Image(systemName: "info")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(.secondary)
                        .frame(width: 46, height: 46)
                        .glassEffect(.regular.interactive(), in: .circle)
                        .glassEffectID("blanket-help", in: glass)
                }
                .buttonStyle(.plain)
                .background(frameReader(id: "blanket-help"))
                .accessibilityLabel("About blanket requirements")
            }
            if blanketsExpanded { board(blanket: true) }
            }
            .padding(.bottom, blanketsExpanded ? 16 : 0)
          }
        }
        .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: {
            boardFrame = $0
        }
        .onPreferenceChange(RequirementChipFrames.self) { chipFrames = $0 }
        .animation(boardSpring, value: requirements)
        .onChange(of: gestureActive) { _, active in
            if !active && lift != nil && !settling { returnLift() }
        }
        .onDisappear { resetLift() }
        .sheet(item: $editor) { presentation in
            RequirementsEditor(
                editing: presentation.key.flatMap { key in requirements.first { $0.key == key } },
                otherRequirements: requirements.filter { $0.key != presentation.key },
                blanket: presentation.blanket,
                editingCount: presentation.count,
                editingTotal: presentation.total,
                editingCopyDepth: presentation.copyDepth,
                onAddResin: presentation.blanket ? nil : {
                    editor = nil
                    Task { @MainActor in
                        try? await Task.sleep(for: .milliseconds(350))
                        resinSource = presentation.source
                        resinPresented = true
                    }
                },
                onEditGroupQuantity: groupQuantityAction(for: presentation),
                onSave: { requirement, count, total, copyDepth in
                    // The board writes the chip and its stack through the
                    // shared core's `save`, which may refuse it; the editor
                    // then stays open and says why.
                    let result = apply([.save(key: presentation.key, requirement: requirement,
                                              count: count, total: total, copyDepth: copyDepth)])
                    if let refusal = result?.refusal { return refusal.message }
                    if presentation.key == nil { land(result?.focus) }
                    return nil
                },
                onRemove: presentation.key.map { key in { remove(key: key) } }
            )
            .navigationTransition(.zoom(sourceID: presentation.source, in: sheetZoom))
        }
        .sheet(isPresented: $resinPresented) {
            RequirementsResinEditor(initialAmount: query.arcaneResin, initialAuto: query.arcaneResinAuto,
                                    initialFilter: query.arcaneResinFilter,
                                    hasRequirement: query.arcaneResinAuto || query.arcaneResin > 0,
                                    onSave: { amount, auto, filter in
                                        query.arcaneResin = amount
                                        query.arcaneResinAuto = auto
                                        query.arcaneResinFilter = filter
                                    }, onRemove: removeResin)
                .navigationTransition(.zoom(sourceID: resinSource, in: sheetZoom))
        }
        .sheet(item: $stackKey) { presentation in
            if let item = snapshot.item(holding: presentation.id) {
                RequirementsStackEditor(count: item.stack.count, copyDepth: item.stack.copyDepth,
                                        range: item.stack.countRange) { count, depth in
                    _ = apply([.setCount(presentation.id, count), .setCopyDepth(presentation.id, depth)])
                }
                .navigationTransition(.zoom(sourceID: presentation.source, in: sheetZoom))
            }
        }
        .alert("Blanket Requirements", isPresented: $blanketHelp) {
            Button("Got it", role: .cancel) {}
        } message: {
            Text("Each blanket must match at least one item fulfilling your ordinary requirements or contributing Arcane Resin. It does not ask for an additional item. All filters in one blanket apply to the same item; separate blankets can match the same or different chosen items.\n\nFor example, require Lightning, Disintegration, and Frost at +2 or higher, then add an Any wand blanket at exactly +3 from the Wandmaker.")
        }
    }

    /// Grouping is a gesture, so it is taught once, only when there are two
    /// chips to group, and never again after the first either/or group.
    private var boardHint: (text: String, symbol: String)? {
        let shown = snapshot
        if shown.ordinaryCount == 0 && !query.arcaneResinAuto && query.arcaneResin == 0 {
            return ("Add the items a seed must contain.", "sparkles")
        }
        guard !learnedGrouping, shown.ordinaryCount >= 2,
              !shown.items.contains(where: { $0.cluster != nil }) else { return nil }
        return ("Hold a chip and drop it on another for either/or.", "hand.draw")
    }

    private var boardSpring: Animation? {
        reduceMotion ? nil : .spring(response: 0.42, dampingFraction: 0.76)
    }

    private func board(blanket: Bool) -> some View {
        RequirementsFlowLayout(spacing: compactChips ? 10 : 12) {
            ForEach(boardEntries(blanket: blanket)) { entry in
                boardEntry(entry.item)
                    .transition(.scale(scale: 0.8).combined(with: .opacity))
            }
            if !blanket, let resin = snapshot.resin {
                resinChip(resin)
            }
            Button {
                editor = RequirementsEditorPresentation(blanket: blanket, source: blanket ? "add-blanket" : "add")
            } label: {
                Label("Add", systemImage: "plus")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(AppTheme.accent)
                    .padding(.horizontal, 20)
                    .frame(minHeight: compactChips ? 46 : 54)
                    .glassEffect(.regular.tint(AppTheme.accent.opacity(0.08)).interactive(), in: .capsule)
                    .glassEffectID(blanket ? "add-blanket" : "add", in: glass)
            }
            .buttonStyle(.plain)
            .matchedTransitionSource(id: blanket ? "add-blanket" : "add", in: sheetZoom)
            .background(frameReader(id: blanket ? "add-blanket" : "add"))
            .accessibilityLabel("Add requirement")
            .opacity(interaction.isDragging ? 0.4 : 1)
            .disabled(interaction.isDragging)
        }
        .padding(.vertical, blanket ? 0 : 8)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func boardEntries(blanket: Bool) -> [RequirementBoardEntry] {
        snapshot.section(blanket: blanket)
            .map { RequirementBoardEntry(id: $0.cluster.map { "group-\($0)" } ?? "chip-\($0.anchor)", item: $0) }
    }

    @ViewBuilder private func boardEntry(_ item: BoardItem) -> some View {
        if let cluster = item.cluster {
            RequirementsFlowLayout(spacing: 8, fillsWidth: false) {
                ForEach(item.chips) { member in
                    HStack(spacing: 8) {
                        chip(member, item: item)
                        if member.key == item.anchor, let badge = item.countBadge, item.stack.canChangeCount {
                            Button {
                                stackKey = RequirementsStackPresentation(id: item.anchor,
                                                                         source: "stack-\(cluster)")
                            } label: {
                                Text(badge.compactText)
                                    .font(.caption.monospaced().weight(.semibold))
                                    .foregroundStyle(AppTheme.seed)
                                    .padding(.horizontal, 12).frame(minHeight: 44)
                                    .glassEffect(.regular.interactive(), in: .capsule)
                            }
                            .buttonStyle(.plain)
                            .matchedTransitionSource(id: "stack-\(cluster)", in: sheetZoom)
                            .background(frameReader(id: "stack-\(cluster)"))
                            .accessibilityLabel("How many")
                        }
                    }
                }
            }
            .padding(8)
            .glassEffect(.regular.tint(AppTheme.seed.opacity(0.10)), in: .rect(cornerRadius: 32))
            .glassEffectID("group-\(cluster)", in: glass)
            .id("group-\(cluster)")
        } else if let member = item.chips.first {
            chip(member, item: item)
        }
    }

    private func resinChip(_ resin: BoardResinChip) -> some View {
        Button {
            guard lift == nil, Date.now >= suppressEditingUntil else { return }
            resinSource = "resin"
            resinPresented = true
        } label: {
            resinContent(resin)
                .glassEffect(.regular.tint(.orange.opacity(0.05)).interactive(), in: .capsule)
                .glassEffectID("resin", in: glass)
        }
        .buttonStyle(.plain)
        .matchedTransitionSource(id: "resin", in: sheetZoom)
        .contentShape(.interaction, Capsule())
        .background(frameReader(id: "resin"))
        .opacity(lift?.id == "resin" ? 0.18 : 1)
        .scaleEffect(lift?.id == "resin" ? 0.94 : 1)
        .simultaneousGesture(liftGesture(id: "resin"))
        .accessibilityLabel(resin.description)
        .accessibilityAction(named: Text("Remove requirement"), removeResin)
    }

    private func resinContent(_ resin: BoardResinChip) -> some View {
        HStack(spacing: 8) {
            ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317),
                           pointSize: compactChips ? 23 : 28)
            Text(resin.name)
                .font(compactChips ? .caption.weight(.medium) : .subheadline.weight(.medium))
                .lineLimit(1).minimumScaleFactor(0.85).layoutPriority(-1)
            ForEach(resin.tags, id: \.self) { value in tag(value.text, upgrade: value.isUpgrade) }
            if resin.uncursed {
                uncursedTag
            }
        }
        .padding(.horizontal, compactChips ? 12 : 15)
        .frame(minHeight: compactChips ? 44 : 52)
    }

    private func chip(_ chip: BoardChip, item: BoardItem) -> some View {
        let id = "chip-\(chip.key)"
        let hovered = hoverKey == chip.key
        return Button {
            guard lift == nil, Date.now >= suppressEditingUntil else { return }
            editor = RequirementsEditorPresentation(key: chip.key, blanket: item.blanket,
                                                   count: item.stack.count, total: item.stack.total,
                                                   copyDepth: item.stack.copyDepth, source: id)
        } label: {
            chipContent(chip, item: item)
                .glassEffect(.regular.tint(chipTint(chip, hovered: hovered)).interactive(), in: .capsule)
                .glassEffectID(id, in: glass)
                .glassEffectUnion(id: id, namespace: glass)
        }
        .buttonStyle(.plain)
        .matchedTransitionSource(id: id, in: sheetZoom)
        .contentShape(.interaction, Capsule())
        .background(frameReader(id: id))
        .opacity(lift?.id == id ? 0.18 : 1)
        .scaleEffect(reduceMotion ? 1 : (lift?.id == id ? 0.94 : (hovered ? 1.045 : (landedKey == chip.key ? 1.06 : 1))))
        .offset(y: !reduceMotion && hovered ? -3 : 0)
        .animation(boardSpring, value: hovered)
        .simultaneousGesture(liftGesture(id: id))
        .accessibilityLabel(chip.problem.map { "\(chip.description), \($0)" } ?? chip.description)
        .accessibilityActions {
            Button(chip.inCluster ? "Remove alternative" : "Remove requirement", role: .destructive) {
                remove(key: chip.key)
            }
            // The rows the core offers this chip, as the drag would.
            ForEach(chip.join, id: \.self) { target in
                Button("or \(chipName(target))") {
                    join(key: chip.key, target: target)
                }
            }
        }
    }

    private func chipName(_ key: Int64) -> String {
        snapshot.chip(key)?.name ?? ""
    }

    private func chipTint(_ chip: BoardChip, hovered: Bool) -> Color {
        // A drop the core refuses warns rather than invites.
        if hovered { return lift?.chip?.refusal(onto: chip.key) == nil ? AppTheme.seed.opacity(0.24) : Color.red.opacity(0.22) }
        if landedKey == chip.key { return AppTheme.accent.opacity(0.4) }
        if chip.problem != nil { return Color.red.opacity(0.14) }
        return chip.inCluster ? AppTheme.seed.opacity(0.04) : .white.opacity(0.015)
    }

    private func chipContent(_ chip: BoardChip, item: BoardItem) -> some View {
        HStack(spacing: compactChips ? 6 : 8) {
            RequirementsChipSprite(chip: chip, size: compactChips ? 23 : 28)
            Text(chip.name)
                .font(compactChips ? .caption.weight(.medium) : .subheadline.weight(.medium))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .layoutPriority(-1)
                .foregroundStyle(.primary)
            HStack(spacing: 4) {
                ForEach(chip.tags, id: \.self) { value in tag(value.text, upgrade: value.isUpgrade) }
                if chip.uncursed {
                    uncursedTag
                }
                if item.cluster == nil, let badge = item.countBadge { tag(badge.compactText) }
                if item.cluster == nil, let badge = item.totalBadge { tag(badge.compactText) }
                RequirementEffectBadge(effect: chip.effect, isWildcard: chip.item == nil)
                ForEach(chip.trailingTags, id: \.self) { value in tag(value.text, upgrade: value.isUpgrade) }
            }
            .fixedSize(horizontal: true, vertical: false)
        }
        .padding(.horizontal, compactChips ? 12 : 15)
        .frame(minHeight: compactChips ? 44 : 52)
    }

    private func frameReader(id: String) -> some View {
        GeometryReader { geometry in
            Color.clear.preference(key: RequirementChipFrames.self,
                                   value: [id: geometry.frame(in: .global)])
        }
    }


    private func liftGesture(id: String) -> some Gesture {
        LongPressGesture(minimumDuration: 0.25, maximumDistance: 12)
            .sequenced(before: DragGesture(minimumDistance: 0, coordinateSpace: .global))
            .updating($gestureActive) { value, state, _ in
                if case .second(true, _) = value { state = true }
            }
            .onChanged { value in
                guard case .second(true, let drag) = value, !settling else { return }
                if lift == nil { beginLift(id: id) }
                guard lift?.id == id else { return }
                if let drag {
                    lift?.translation = drag.translation
                    interaction.location = drag.location
                    updateHover(at: drag.location)
                }
            }
            .onEnded { value in
                guard case .second(true, _) = value, lift?.id == id else { return }
                dropLift()
            }
    }

    private func beginLift(id: String) {
        guard let frame = chipFrames[id] else { return }
        let shown = snapshot
        let key = id.hasPrefix("chip-") ? Int64(id.dropFirst(5)) : nil
        // The chip carries its join candidates and refusals, read once here
        // rather than on every frame of the drag.
        let lifted = key.flatMap { shown.chip($0) }
        let item = key.flatMap { shown.item(holding: $0) }
        if let lifted, let item {
            interaction.preview = AnyView(chipContent(lifted, item: item))
        } else if let resin = shown.resin {
            interaction.preview = AnyView(resinContent(resin))
        }
        liftGeneration = UUID()
        suppressEditingUntil = .distantFuture
        settling = false
        withAnimation(boardSpring) {
            lift = RequirementLift(id: id, frame: frame, chip: lifted, item: item,
                                   scale: reduceMotion ? 1 : 1.06)
            interaction.activeID = id
            interaction.location = CGPoint(x: frame.midX, y: frame.midY)
        }
        UIImpactFeedbackGenerator(style: .soft).impactOccurred()
    }

    private func updateHover(at point: CGPoint) {
        let previous = hoverKey
        let overRemove = interaction.isOverRemove
        guard !overRemove, let source = lift?.chip else {
            if previous != nil { withAnimation(boardSpring) { hoverKey = nil } }
            return
        }
        // Only the rows the core names — joins, and refusals to explain —
        // answer the finger.
        let candidates = source.join + source.refuse.map(\.key)
        let target = candidates.first { candidate in
            chipFrames["chip-\(candidate)"]?.insetBy(dx: -6, dy: -6).contains(point) == true
        }
        if target != previous {
            withAnimation(boardSpring) { hoverKey = target }
            if target != nil { UISelectionFeedbackGenerator().selectionChanged() }
        }
    }

    private func dropLift() {
        guard let current = lift, !settling else { return }
        settling = true
        let generation = liftGeneration
        if interaction.isOverRemove {
            let destination = interaction.removeFrame
            withAnimation(reduceMotion ? .easeOut(duration: 0.12) : .spring(response: 0.25, dampingFraction: 0.8)) {
                lift?.translation = CGSize(width: destination.midX - current.frame.midX,
                                           height: destination.midY - current.frame.midY)
                lift?.scale = reduceMotion ? 1 : 0.05
                lift?.opacity = 0
                hoverKey = nil
            }
            UIImpactFeedbackGenerator(style: .rigid).impactOccurred()
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(200))
                guard generation == liftGeneration else { return }
                if let key = current.chip?.key { remove(key: key) } else { removeResin() }
                resetLift()
            }
        } else if let target = hoverKey, let source = current.chip, let refusal = source.refusal(onto: target) {
            UINotificationFeedbackGenerator().notificationOccurred(.warning)
            show(refusal.message)
            returnLift()
        } else if let target = hoverKey, let source = current.chip {
            withAnimation(boardSpring) {
                if apply([.join(source: source.key, target: target)])?.changed == true {
                    learnedGrouping = true
                }
                resetLift()
            }
            UIImpactFeedbackGenerator(style: .soft).impactOccurred(intensity: 0.8)
        } else if let source = current.chip, source.canDetach,
                  boardFrame.contains(interaction.location),
                  !current.frame.insetBy(dx: -12, dy: -12).contains(interaction.location),
                  !chipFrames.contains(where: { $0.key != current.id && $0.value.contains(interaction.location) }) {
            withAnimation(boardSpring) {
                detach(key: source.key)
                resetLift()
            }
            UISelectionFeedbackGenerator().selectionChanged()
        } else {
            returnLift()
        }
    }

    private func returnLift() {
        guard lift != nil else { return }
        settling = true
        let generation = liftGeneration
        withAnimation(boardSpring) {
            lift?.translation = .zero
            lift?.scale = 1
            hoverKey = nil
        }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(reduceMotion ? 0 : 220))
            guard generation == liftGeneration else { return }
            withAnimation(.easeOut(duration: 0.12)) { resetLift() }
        }
    }

    private func resetLift() {
        // A simultaneous Button tap can arrive after the drag's onEnded. Keep
        // that release from opening an editor after a successful group/drop.
        suppressEditingUntil = .now.addingTimeInterval(0.35)
        lift = nil
        hoverKey = nil
        settling = false
        interaction.reset()
        liftGeneration = UUID()
    }

    private func tag(_ text: String, upgrade: Bool = false) -> some View {
        let color = upgrade ? AppTheme.upgrade : AppTheme.seed
        return Text(text).font(.caption2.monospaced().weight(upgrade ? .bold : .semibold))
            .foregroundStyle(color).padding(.horizontal, 5).padding(.vertical, 2)
            .background(color.opacity(upgrade ? 0.12 : 0.14), in: Capsule())
    }

    /// Uncursed is a green checkmark in the same capsule as the text tags.
    private var uncursedTag: some View {
        Text(Image(systemName: "checkmark")).font(.caption2.weight(.bold))
            .foregroundStyle(AppTheme.softGreen).padding(.horizontal, 5).padding(.vertical, 2)
            .background(AppTheme.softGreen.opacity(0.14), in: Capsule())
            .accessibilityLabel("Uncursed")
    }

    /// A cluster's stack is the cluster's, so its member's editor hands
    /// "How many" over to the stack sheet — while the core offers a count.
    private func groupQuantityAction(for presentation: RequirementsEditorPresentation) -> (() -> Void)? {
        guard let key = presentation.key, let item = snapshot.item(holding: key),
              item.cluster != nil, item.stack.canChangeCount else { return nil }
        return {
            editor = nil
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(350))
                guard let current = snapshot.item(holding: key), current.stack.canChangeCount else { return }
                stackKey = RequirementsStackPresentation(id: key, source: "chip-\(key)")
            }
        }
    }

    /// A new chip lands lit: its glass takes on the accent as the editor
    /// folds away, then cools to the board's own tint.
    private func land(_ key: Int64?) {
        guard let key else { return }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(reduceMotion ? 0 : 280))
            withAnimation(boardSpring) { landedKey = key }
            try? await Task.sleep(for: .milliseconds(900))
            guard landedKey == key else { return }
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.8)) { landedKey = nil }
        }
    }

    /// Runs board edits through the shared core, writing the rows back only
    /// when they changed: an edit that did nothing leaves the query — and
    /// the search it would resume or refine — untouched.
    @discardableResult
    private func apply(_ edits: [BoardEdit]) -> RequirementBoard? {
        guard let result = RequirementBoard.apply(edits, to: requirements, resin: query.boardResin) else { return nil }
        if result.changed {
            withAnimation(boardSpring) { query.requirements = result.rows }
            // Keys the core had to repair carry the chip the board is
            // following along with them.
            landedKey = landedKey.map { result.key(following: $0) }
        }
        return result
    }

    private func join(key: Int64, target: Int64) {
        if let refusal = apply([.join(source: key, target: target)])?.refusal { show(refusal.message) }
    }

    private func detach(key: Int64) {
        apply([.detach(key)])
    }

    private func remove(key: Int64) {
        apply([.remove(key)])
    }

    /// Says why a drop could not join, in the hint's place, for a moment.
    private func show(_ message: String) {
        withAnimation(boardSpring) { notice = message }
        noticeReset?.cancel()
        noticeReset = Task { @MainActor in
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled else { return }
            withAnimation(boardSpring) { notice = nil }
        }
    }

    private func removeResin() {
        withAnimation(boardSpring) {
            query.arcaneResin = 0
            query.arcaneResinAuto = false
            query.arcaneResinFilter = ArcaneResinFilter()
        }
    }
}

private struct RequirementBoardEntry: Identifiable {
    let id: String
    let item: BoardItem
}


private struct RequirementChipFrames: PreferenceKey {
    static let defaultValue: [String: CGRect] = [:]
    static func reduce(value: inout [String: CGRect], nextValue: () -> [String: CGRect]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}

private struct RequirementsEditorPresentation: Identifiable {
    var id = UUID()
    var key: Int64?
    var blanket: Bool
    var count = 1
    var total: Int?
    var copyDepth: Int?
    /// The chip or Add button the editor grows from.
    var source: String
}

private struct RequirementsStackPresentation: Identifiable {
    let id: Int64
    let source: String
}

/// A board chip's sprite: the item with its effects' glows, or the
/// wildcard's family silhouette.
struct RequirementsChipSprite: View {
    let chip: BoardChip
    var size: Int = 32
    var body: some View {
        Group {
            if let item = chip.catalogItem {
                ItemSpriteView(item: item, glows: chipGlows(chip.effect), pointSize: size)
            } else if let kind = chip.kind {
                WildcardSpriteView(kind: kind, pointSize: size)
            }
        }
        .frame(width: CGFloat(size), height: CGFloat(size))
        .accessibilityHidden(true)
    }
}

struct RequirementsSprite: View {
    let requirement: ItemRequirement
    var size: Int = 32
    var body: some View {
        Group {
            if let item = requirement.item {
                ItemSpriteView(item: item, glows: requirementGlows(requirement.effect), pointSize: size)
            } else {
                WildcardSpriteView(kind: requirement.kind, pointSize: size)
            }
        }
        .frame(width: CGFloat(size), height: CGFloat(size))
        .accessibilityHidden(true)
    }
}

/// Chips wrap as one board, and each either/or capsule wraps its own contents.
struct RequirementsFlowLayout: Layout {
    var spacing: CGFloat = 7
    var fillsWidth = true

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = availableWidth(proposal, subviews: subviews)
        let size = arrange(subviews, width: width).size
        return CGSize(width: fillsWidth ? width : size.width, height: size.height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        // Use the proposal from measurement, not its possibly smaller intrinsic
        // result. Otherwise nested flows gain rows after their height is fixed.
        let result = arrange(subviews, width: availableWidth(proposal, subviews: subviews))
        for (index, frame) in result.frames.enumerated() {
            subviews[index].place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                                  proposal: ProposedViewSize(frame.size))
        }
    }

    private func availableWidth(_ proposal: ProposedViewSize, subviews: Subviews) -> CGFloat {
        if let width = proposal.width, width.isFinite { return max(1, width) }
        return max(1, subviews.reduce(CGFloat.zero) { $0 + $1.sizeThatFits(.unspecified).width }
                   + spacing * CGFloat(max(0, subviews.count - 1)))
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> (frames: [CGRect], size: CGSize) {
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var usedWidth: CGFloat = 0
        var frames: [CGRect] = []
        var rowStart = 0
        func centerRow() {
            for index in rowStart..<frames.count {
                frames[index].origin.y += (rowHeight - frames[index].height) / 2
            }
        }
        for subview in subviews {
            let ideal = subview.sizeThatFits(.unspecified)
            let childWidth = min(ideal.width, width)
            let size = subview.sizeThatFits(ProposedViewSize(width: childWidth, height: nil))
            if x > 0 && x + childWidth > width {
                centerRow()
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
                rowStart = frames.count
            }
            frames.append(CGRect(x: x, y: y, width: childWidth, height: size.height))
            usedWidth = max(usedWidth, x + childWidth)
            x += childWidth + spacing
            rowHeight = max(rowHeight, size.height)
        }
        centerRow()
        return (frames, CGSize(width: usedWidth, height: y + rowHeight))
    }
}

private struct RequirementsStackEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State var count: Int
    @State var copyDepth: Int?
    /// The counts the core offers: up to the stack limit while the cluster
    /// can grow, else only down from its count.
    let range: ClosedRange<Int>
    let onSave: (Int, Int?) -> Void

    var body: some View {
        NavigationStack {
            Form {
                Stepper(value: $count, in: range) {
                    HStack { Text("How many"); Spacer(); Text("×\(count)").foregroundStyle(.tint) }
                }
                if count > 1 {
                    Section {
                        Toggle("Limit the extra copies to a floor", isOn: Binding(get: { copyDepth != nil }, set: { copyDepth = $0 ? 4 : nil }))
                        if copyDepth != nil { RequirementsFloorPicker(title: "Copies within first", depth: $copyDepth, allowsNone: false) }
                    } footer: {
                        Text("A floor limit is where an item lies, not what it is, so the copies keep their own.")
                    }
                }
            }
            .navigationTitle("How many")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { onSave(count, count > 1 ? copyDepth : nil); dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
