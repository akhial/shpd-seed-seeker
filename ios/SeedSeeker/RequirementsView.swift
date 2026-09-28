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
    @State private var resinEditor: RequirementsResinPresentation?
    @State private var blanketsExpanded = false
    @State private var blanketHelp = false
    @State private var chipFrames: [String: CGRect] = [:]
    @State private var boardFrame = CGRect.zero
    @State private var hoverKey: Int64?
    @State private var settling = false
    @State private var liftGeneration = UUID()
    @State private var suppressEditingUntil = Date.distantPast
    @State private var stackKey: RequirementsStackPresentation?
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
                sheet: presentation.sheet,
                onPickResin: { picked in
                    // Arcane Resin has a sheet of its own; the picked draft
                    // moves there, with the wand chip it may replace.
                    editor = nil
                    Task { @MainActor in
                        try? await Task.sleep(for: .milliseconds(350))
                        resinEditor = RequirementsResinPresentation(sheet: picked, source: presentation.source)
                    }
                },
                onEditGroupQuantity: groupQuantityAction(for: presentation),
                onSave: { save($0) },
                onRemove: presentation.key.map { key in { remove(key: key) } }
            )
            .navigationTransition(.zoom(sourceID: presentation.source, in: sheetZoom))
        }
        .sheet(item: $resinEditor) { presentation in
            RequirementsResinEditor(sheet: presentation.sheet,
                                    hasRequirement: query.arcaneResinAuto || query.arcaneResin > 0,
                                    onSave: { save($0) }, onRemove: removeResin)
                .navigationTransition(.zoom(sourceID: presentation.source, in: sheetZoom))
        }
        .sheet(item: $stackKey) { presentation in
            if let item = snapshot.item(holding: presentation.id) {
                RequirementsStackEditor(key: presentation.id, stack: item.stack,
                                        copyFloor: presentation.copyFloor) { edit in
                    editStack(presentation.id, edit)
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
                openEditor(RequirementSheet.open(rows: requirements, blanket: blanket, resin: query.boardResin,
                                                 offerResin: true),
                           key: nil, source: blanket ? "add-blanket" : "add")
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
                                showStack(item.anchor, source: "stack-\(cluster)")
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
            guard lift == nil, Date.now >= suppressEditingUntil,
                  let sheet = RequirementSheet.open(rows: requirements, resin: query.boardResin,
                                                    openResin: true) else { return }
            resinEditor = RequirementsResinPresentation(sheet: sheet, source: "resin")
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
            // The resin it counts (style credit) keeps the seed tint iOS has
            // always drawn every resin tag in. Tags have no hover here; what
            // "Mage +2" means is in the description VoiceOver reads.
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
            openEditor(RequirementSheet.open(rows: requirements, key: chip.key, resin: query.boardResin,
                                             offerResin: true),
                       key: chip.key, source: id)
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
                showStack(key, source: "chip-\(key)")
            }
        }
    }

    /// Opens a cluster's "How many" sheet on its anchor `key`.
    private func showStack(_ key: Int64, source: String) {
        stackKey = RequirementsStackPresentation(id: key, source: source,
                                                 copyFloor: copyFloor(of: key, in: requirements))
    }

    /// Runs one edit of the cluster's "How many" sheet on the board as it is
    /// made, and answers what the sheet shows next: why the core refused it,
    /// and the copies' floor control after it.
    private func editStack(_ key: Int64, _ edit: BoardEdit) -> (refusal: String?, copyFloor: SheetFloorToggle?) {
        let result = apply([edit])
        if result?.refusal != nil { UINotificationFeedbackGenerator().notificationOccurred(.warning) }
        return (result?.refusal?.message, copyFloor(of: key, in: result?.rows ?? requirements))
    }

    /// The copies' floor control as the shared core words it for the chip's
    /// own sheet: its switch, the floors it stops at and the floor it turns
    /// on at. A cluster member's sheet hides it — the stack is the cluster's,
    /// and the "How many" sheet edits it on the board — but the core fills
    /// it in all the same.
    private func copyFloor(of key: Int64, in rows: [ItemRequirement]) -> SheetFloorToggle? {
        RequirementSheet.open(rows: rows, key: key)?.form.stack.copyDepth
    }

    /// Shows a sheet the shared core opened as the chip or "Add" was tapped,
    /// never from the sheet's own builder, which runs again on every update.
    private func openEditor(_ sheet: RequirementSheet?, key: Int64?, source: String) {
        guard let sheet else { return }
        editor = RequirementsEditorPresentation(sheet: sheet, key: key, source: source)
    }

    /// Saves a sheet onto the list as it is now, through the shared core:
    /// the rows it writes back — only when they changed — and the query's
    /// resin when the sheet set or cleared it. Answers the sheet to keep
    /// showing when the core refused the save; its errors say why.
    private func save(_ sheet: RequirementSheet) -> RequirementSheet? {
        guard let outcome = sheet.save(onto: requirements) else { return sheet }
        switch outcome {
        case .refused(let refused):
            return refused
        case .saved(let saved):
            withAnimation(boardSpring) {
                if saved.changed { query.requirements = saved.rows }
                switch saved.resin {
                case .set(let condition):
                    query.arcaneResin = condition.amount
                    query.arcaneResinAuto = condition.auto
                    query.arcaneResinFilter = condition.filter
                case .clear:
                    query.arcaneResin = 0
                    query.arcaneResinAuto = false
                    query.arcaneResinFilter = ArcaneResinFilter()
                case .unchanged:
                    break
                }
            }
            // Keys the core had to repair carry the chip the board is
            // following along with them.
            if saved.changed { landedKey = landedKey.map { saved.key(following: $0) } }
            if sheet.form.mode == .new { land(saved.focus) }
            return nil
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
    let id = UUID()
    let sheet: RequirementSheet
    /// The row the sheet was opened on, nil for a new chip.
    let key: Int64?
    /// The chip or Add button the editor grows from.
    let source: String
}

private struct RequirementsResinPresentation: Identifiable {
    let id = UUID()
    let sheet: RequirementSheet
    /// The resin chip, or the chip whose sheet picked Arcane Resin.
    let source: String
}

private struct RequirementsStackPresentation: Identifiable {
    let id: Int64
    let source: String
    /// The copies' floor control as the sheet opens.
    let copyFloor: SheetFloorToggle?
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

/// The "How many" sheet of an either/or cluster, whose stack is the
/// cluster's rather than any one member's. Every control is one board edit,
/// applied as it is made, so the sheet always shows the stack the board holds.
private struct RequirementsStackEditor: View {
    @Environment(\.dismiss) private var dismiss
    /// The cluster's anchor, which the edits name.
    let key: Int64
    /// The cluster's stack as the board draws it now.
    let stack: BoardStack
    /// The copies' floor control, in the shared core's words.
    @State var copyFloor: SheetFloorToggle?
    /// Runs one edit; answers why the core refused it, and the copies' floor
    /// control after it.
    let onEdit: (BoardEdit) -> (refusal: String?, copyFloor: SheetFloorToggle?)
    /// Why the core refused the last edit.
    @State private var refusal: String?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Stepper(value: countBinding, in: stack.countRange) {
                        HStack { Text("How many"); Spacer(); Text(stack.countText).foregroundStyle(.tint) }
                    }
                } footer: {
                    if let refusal { Text(refusal).foregroundStyle(.orange) }
                }
                if stack.canSetCopyDepth, let copyFloor {
                    Section {
                        RequirementsFloorControl(control: copyFloor, enabled: copyFloorEnabled(copyFloor),
                                                 floor: copyFloorValue(copyFloor))
                    } footer: {
                        Text("A floor limit is where an item lies, not what it is, so the copies keep their own.")
                    }
                }
            }
            .navigationTitle("How many")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } }
            }
        }
        .presentationDetents([.medium, .large])
    }

    private var countBinding: Binding<Int> {
        Binding(get: { stack.count }, set: { count in
            if count != stack.count { run(.setCount(key, count)) }
        })
    }

    /// Turning the limit on starts the copies at the floor the core offers.
    private func copyFloorEnabled(_ control: SheetFloorToggle) -> Binding<Bool> {
        Binding(get: { control.enabled }, set: { on in
            if on != control.enabled { run(.setCopyDepth(key, on ? control.value : nil)) }
        })
    }

    /// A slider stop the copies are not already at.
    private func copyFloorValue(_ control: SheetFloorToggle) -> Binding<Int> {
        Binding(get: { control.value }, set: { floor in
            if floor != control.value { run(.setCopyDepth(key, floor)) }
        })
    }

    private func run(_ edit: BoardEdit) {
        let answer = onEdit(edit)
        refusal = answer.refusal
        copyFloor = answer.copyFloor
    }
}
