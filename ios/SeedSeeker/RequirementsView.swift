import SwiftUI
import SeedSeekerKit
import UIKit

/// The board operates on the shared relation model, including the hidden copies
/// behind a stack. Keys, rather than positions, survive every model rewrite.
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

    private var requirements: [ItemRequirement] { query.requirements }
    private var lift: RequirementLift? {
        get { interaction.lift }
        nonmutating set { interaction.lift = newValue }
    }

    var body: some View {
        GlassEffectContainer(spacing: 10) {
          VStack(alignment: .leading, spacing: 18) {
            board(blanket: false)
            VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Button {
                    withAnimation(boardSpring) { blanketsExpanded.toggle() }
                } label: {
                    HStack(spacing: 9) {
                        Image(systemName: "square.3.layers.3d")
                            .font(.subheadline)
                        Text("Blanket Requirements (\(requirements.filter(\.blanket).boardCount))")
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
                        resinPresented = true
                    }
                },
                onEditGroupQuantity: groupQuantityAction(for: presentation),
                onSave: { requirement, count, total, copyDepth in
                    let index = presentation.key.flatMap { key in requirements.firstIndex { $0.key == key } }
                    withAnimation(boardSpring) {
                        query.requirements = requirements.applyEdit(index: index, requirement: requirement,
                                                                   count: count, total: total, copyDepth: copyDepth)
                    }
                },
                onRemove: presentation.key.map { key in { remove(key: key) } }
            )
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
        }
        .sheet(item: $stackKey) { presentation in
            if let index = requirements.firstIndex(where: { $0.key == presentation.id }),
               let item = requirements.boardItem(holding: index) {
                RequirementsStackEditor(count: item.stackCount, copyDepth: requirements.copyDepth(of: item)) { count, depth in
                    let updated = requirements.setStackCount(item, count)
                    guard let updatedIndex = updated.firstIndex(where: { $0.key == presentation.id }),
                          let refreshed = updated.boardItem(holding: updatedIndex) else { return }
                    query.requirements = updated.setCopyDepth(refreshed, depth)
                }
            }
        }
        .alert("Blanket Requirements", isPresented: $blanketHelp) {
            Button("Got it", role: .cancel) {}
        } message: {
            Text("Each blanket must match at least one item fulfilling your ordinary requirements or contributing Arcane Resin. It does not ask for an additional item. All filters in one blanket apply to the same item; separate blankets can match the same or different chosen items.\n\nFor example, require Lightning, Disintegration, and Frost at +2 or higher, then add an Any wand blanket at exactly +3 from the Wandmaker.")
        }
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
            if !blanket && (query.arcaneResinAuto || query.arcaneResin > 0) {
                resinChip
            }
            Button {
                editor = RequirementsEditorPresentation(blanket: blanket)
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
            .background(frameReader(id: blanket ? "add-blanket" : "add"))
            .accessibilityLabel("Add requirement")
            .opacity(interaction.isDragging ? 0.4 : 1)
            .disabled(interaction.isDragging)
        }
        .padding(.vertical, blanket ? 0 : 8)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func boardEntries(blanket: Bool) -> [RequirementBoardEntry] {
        requirements.boardItems()
            .filter { requirements[$0.anchor].blanket == blanket }
            .map { RequirementBoardEntry(id: $0.cluster.map { "group-\($0)" } ?? "chip-\(requirements[$0.anchor].key)", item: $0) }
    }

    @ViewBuilder private func boardEntry(_ item: BoardItem) -> some View {
        if let cluster = item.cluster {
            RequirementsFlowLayout(spacing: 8, fillsWidth: false) {
                ForEach(item.members.map { requirements[$0] }, id: \.key) { requirement in
                    HStack(spacing: 8) {
                        chip(requirement, item: item)
                        if requirement.key == requirements[item.anchor].key && item.stackCount > 1 && requirements.canStack(item) {
                            Button {
                                stackKey = RequirementsStackPresentation(id: requirements[item.anchor].key)
                            } label: {
                                Text("×\(item.stackCount)")
                                    .font(.caption.monospaced().weight(.semibold))
                                    .foregroundStyle(AppTheme.seed)
                                    .padding(.horizontal, 12).frame(minHeight: 44)
                                    .glassEffect(.regular.interactive(), in: .capsule)
                            }
                            .buttonStyle(.plain)
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
        } else {
            chip(requirements[item.anchor], item: item)
        }
    }

    private var resinChip: some View {
        Button {
            guard lift == nil, Date.now >= suppressEditingUntil else { return }
            resinPresented = true
        } label: {
            resinContent
                .glassEffect(.regular.tint(.orange.opacity(0.05)).interactive(), in: .capsule)
                .glassEffectID("resin", in: glass)
        }
        .buttonStyle(.plain)
        .contentShape(.interaction, Capsule())
        .background(frameReader(id: "resin"))
        .opacity(lift?.id == "resin" ? 0.18 : 1)
        .scaleEffect(lift?.id == "resin" ? 0.94 : 1)
        .simultaneousGesture(liftGesture(id: "resin"))
        .accessibilityLabel("Arcane Resin, \(query.arcaneResinAuto ? "Auto, upgrade matched wands to +3" : "at least \(query.arcaneResin)"), \(query.arcaneResinFilter.summary)")
        .accessibilityAction(named: Text("Remove requirement"), removeResin)
    }

    private var resinContent: some View {
        HStack(spacing: 8) {
            ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317),
                           pointSize: compactChips ? 23 : 28)
            Text("Arcane Resin")
                .font(compactChips ? .caption.weight(.medium) : .subheadline.weight(.medium))
                .lineLimit(1).minimumScaleFactor(0.85).layoutPriority(-1)
            tag(query.arcaneResinAuto ? "Auto" : "≥\(query.arcaneResin)")
            if query.arcaneResinFilter.includeMageWand { tag("Mage +2") }
            if let depth = query.arcaneResinFilter.maximumDepth { tag("F≤\(depth)") }
            if query.arcaneResinFilter.uncursed {
                Image(systemName: "shield.lefthalf.filled").font(.caption2).foregroundStyle(AppTheme.softGreen)
            }
        }
        .padding(.horizontal, compactChips ? 12 : 15)
        .frame(minHeight: compactChips ? 44 : 52)
    }

    private func chip(_ requirement: ItemRequirement, item: BoardItem) -> some View {
        let id = "chip-\(requirement.key)"
        let hovered = hoverKey == requirement.key
        return Button {
            guard lift == nil, Date.now >= suppressEditingUntil else { return }
            editor = RequirementsEditorPresentation(key: requirement.key, blanket: requirement.blanket,
                                                   count: item.stackCount, total: item.total,
                                                   copyDepth: requirements.copyDepth(of: item))
        } label: {
            chipContent(requirement, item: item)
                .glassEffect(.regular.tint(chipTint(requirement, hovered: hovered)).interactive(), in: .capsule)
                .glassEffectID(id, in: glass)
                .glassEffectUnion(id: id, namespace: glass)
        }
        .buttonStyle(.plain)
        .contentShape(.interaction, Capsule())
        .background(frameReader(id: id))
        .opacity(lift?.id == id ? 0.18 : 1)
        .scaleEffect(reduceMotion ? 1 : (lift?.id == id ? 0.94 : (hovered ? 1.045 : 1)))
        .offset(y: !reduceMotion && hovered ? -3 : 0)
        .animation(boardSpring, value: hovered)
        .simultaneousGesture(liftGesture(id: id))
        .accessibilityLabel(requirement.title + ", " + requirement.description)
        .accessibilityActions {
            Button(requirement.alternativeGroup == nil ? "Remove requirement" : "Remove alternative", role: .destructive) {
                remove(key: requirement.key)
            }
            ForEach(requirements.filter {
                $0.key != requirement.key && $0.blanket == requirement.blanket
                    && (requirement.alternativeGroup == nil || $0.alternativeGroup != requirement.alternativeGroup)
            }, id: \.key) { target in
                Button("or \(target.item?.name ?? "Any \(target.kind.singularLabel)")") {
                    join(key: requirement.key, target: target.key)
                }
            }
        }
    }

    private func chipTint(_ requirement: ItemRequirement, hovered: Bool) -> Color {
        if hovered { return AppTheme.seed.opacity(0.24) }
        return requirement.alternativeGroup != nil ? AppTheme.seed.opacity(0.04) : .white.opacity(0.015)
    }

    private func chipContent(_ requirement: ItemRequirement, item: BoardItem) -> some View {
        HStack(spacing: compactChips ? 6 : 8) {
            RequirementsSprite(requirement: requirement, size: compactChips ? 23 : 28)
            Text(requirement.item?.name ?? "Any \(requirement.kind.singularLabel)")
                .font(compactChips ? .caption.weight(.medium) : .subheadline.weight(.medium))
                .lineLimit(1)
                .minimumScaleFactor(0.85)
                .layoutPriority(-1)
                .foregroundStyle(.primary)
            HStack(spacing: 4) {
                ForEach(tags(for: requirement), id: \.text) { value in tag(value.text, upgrade: value.upgrade) }
                if requirement.requireUncursed {
                    Image(systemName: "shield.lefthalf.filled").font(.caption2).foregroundStyle(AppTheme.softGreen)
                }
                if item.cluster == nil && item.stackCount > 1 {
                    tag(item.total == nil ? "×\(item.stackCount)" : "≤\(item.stackCount)")
                }
                if let total = item.total, item.cluster == nil { tag("Σ≥\(total)") }
                RequirementEffectBadge(effect: requirement.effect, isWildcard: requirement.item == nil)
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
        let key = id.hasPrefix("chip-") ? Int64(id.dropFirst(5)) : nil
        let index = key.flatMap { key in requirements.firstIndex { $0.key == key } }
        if let index, let item = requirements.boardItem(holding: index) {
            interaction.preview = AnyView(chipContent(requirements[index], item: item))
        } else {
            interaction.preview = AnyView(resinContent)
        }
        liftGeneration = UUID()
        suppressEditingUntil = .distantFuture
        settling = false
        withAnimation(boardSpring) {
            lift = RequirementLift(id: id, frame: frame,
                                   requirement: index.map { requirements[$0] },
                                   item: index.flatMap { requirements.boardItem(holding: $0) },
                                   scale: reduceMotion ? 1 : 1.06)
            interaction.activeID = id
            interaction.location = CGPoint(x: frame.midX, y: frame.midY)
        }
        UIImpactFeedbackGenerator(style: .soft).impactOccurred()
    }

    private func updateHover(at point: CGPoint) {
        let previous = hoverKey
        let overRemove = interaction.isOverRemove
        guard !overRemove, let source = lift?.requirement else {
            if previous != nil { withAnimation(boardSpring) { hoverKey = nil } }
            return
        }
        let target = requirements.first { candidate in
            candidate.key != source.key && candidate.blanket == source.blanket
                && (source.alternativeGroup == nil || candidate.alternativeGroup != source.alternativeGroup)
                && chipFrames["chip-\(candidate.key)"]?.insetBy(dx: -6, dy: -6).contains(point) == true
        }?.key
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
                if let key = current.requirement?.key { remove(key: key) } else { removeResin() }
                resetLift()
            }
        } else if let target = hoverKey, let key = current.requirement?.key {
            withAnimation(boardSpring) {
                if let sourceIndex = requirements.firstIndex(where: { $0.key == key }),
                   let targetIndex = requirements.firstIndex(where: { $0.key == target }) {
                    query.requirements = requirements.joinAlternatives(source: sourceIndex, target: targetIndex)
                }
                resetLift()
            }
            UIImpactFeedbackGenerator(style: .soft).impactOccurred(intensity: 0.8)
        } else if let requirement = current.requirement, requirement.alternativeGroup != nil,
                  boardFrame.contains(interaction.location),
                  !current.frame.insetBy(dx: -12, dy: -12).contains(interaction.location),
                  !chipFrames.contains(where: { $0.key != current.id && $0.value.contains(interaction.location) }) {
            withAnimation(boardSpring) {
                detach(key: requirement.key)
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

    private func tags(for requirement: ItemRequirement) -> [(text: String, upgrade: Bool)] {
        var values: [(text: String, upgrade: Bool)] = []
        switch requirement.tierMatch {
        case .any: break
        case .exactly: values.append(("T\(requirement.tier)", false))
        case .atLeast: values.append(("T\(requirement.tier)+", false))
        case .atMost: values.append(("T≤\(requirement.tier)", false))
        }
        switch requirement.upgradeMatch {
        case .any: break
        case .exactly: values.append(("+\(requirement.upgrade)", true))
        case .atLeast: values.append(("+\(requirement.upgrade)↑", true))
        }
        if requirement.excludeResin { values.append(("No resin", false)) }
        if requirement.trinketTransmutations > 0 { values.append(("Transmute ≤\(requirement.trinketTransmutations)", false)) }
        if requirement.artifactTransmutations > 0 { values.append(("Transmute ≤\(requirement.artifactTransmutations)", false)) }
        if let floor = requirement.maximumDepth { values.append(("F≤\(floor)", false)) }
        return values
    }

    private func groupQuantityAction(for presentation: RequirementsEditorPresentation) -> (() -> Void)? {
        guard let key = presentation.key,
              let index = requirements.firstIndex(where: { $0.key == key }),
              let item = requirements.boardItem(holding: index),
              item.cluster != nil, requirements.canStack(item) else { return nil }
        return {
            editor = nil
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(350))
                guard let currentIndex = requirements.firstIndex(where: { $0.key == key }),
                      let currentItem = requirements.boardItem(holding: currentIndex),
                      requirements.canStack(currentItem) else { return }
                stackKey = RequirementsStackPresentation(id: key)
            }
        }
    }

    private func join(key: Int64, target: Int64) {
        guard let source = requirements.firstIndex(where: { $0.key == key }),
              let target = requirements.firstIndex(where: { $0.key == target }),
              requirements[source].blanket == requirements[target].blanket else { return }
        withAnimation(boardSpring) {
            query.requirements = requirements.joinAlternatives(source: source, target: target)
        }
    }

    private func detach(key: Int64) {
        guard let index = requirements.firstIndex(where: { $0.key == key }) else { return }
        withAnimation(boardSpring) { query.requirements = requirements.detach(index) }
    }

    private func remove(key: Int64) {
        guard let index = requirements.firstIndex(where: { $0.key == key }),
              let item = requirements.boardItem(holding: index) else { return }
        withAnimation(boardSpring) {
            query.requirements = item.cluster == nil ? requirements.removeItem(item) : requirements.removeMember(index)
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
}

private struct RequirementsStackPresentation: Identifiable { let id: Int64 }

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
    let onSave: (Int, Int?) -> Void

    var body: some View {
        NavigationStack {
            Form {
                Stepper(value: $count, in: 1...SearchLimits.stackMax) {
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
