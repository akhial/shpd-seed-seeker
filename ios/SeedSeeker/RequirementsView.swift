import SwiftUI
import UniformTypeIdentifiers
import SeedSeekerKit

/// The board operates on the shared relation model, including the hidden copies
/// behind a stack. Keys, rather than positions, survive every model rewrite.
struct RequirementsView: View {
    @Binding var query: SavedQuery
    @AppStorage("compactChips") private var compactChips = false
    @State private var editor: RequirementsEditorPresentation?
    @State private var resinPresented = false
    @State private var blanketsExpanded = false
    @State private var blanketHelp = false
    @State private var draggingKey: Int64?
    @State private var draggingResin = false
    @State private var stackKey: RequirementsStackPresentation?

    private var requirements: [ItemRequirement] { query.requirements }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            board(blanket: false)
            HStack(spacing: 8) {
                Button {
                    withAnimation { blanketsExpanded.toggle() }
                } label: {
                    HStack {
                        Text("Blanket Requirements (\(requirements.filter(\.blanket).boardCount))")
                        Spacer()
                        Image(systemName: blanketsExpanded ? "chevron.up" : "chevron.down")
                    }
                    .font(.subheadline.weight(.medium))
                    .padding(.vertical, 10)
                }
                .accessibilityHint(blanketsExpanded ? "Collapse blankets" : "Expand blankets")
                Button { blanketHelp = true } label: {
                    Image(systemName: "info.circle").padding(8)
                }
                .accessibilityLabel("About blanket requirements")
            }
            if blanketsExpanded { board(blanket: true) }
        }
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
                onSave: { requirement, count, total, copyDepth in
                    let index = presentation.key.flatMap { key in requirements.firstIndex { $0.key == key } }
                    query.requirements = requirements.applyEdit(index: index, requirement: requirement,
                                                               count: count, total: total, copyDepth: copyDepth)
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

    private func board(blanket: Bool) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            RequirementsFlowLayout(spacing: compactChips ? 5 : 7) {
                ForEach(requirements.boardItems().filter { requirements[$0.anchor].blanket == blanket }) { item in
                    entry(item)
                }
                if !blanket && (query.arcaneResinAuto || query.arcaneResin > 0) {
                    Button { resinPresented = true } label: {
                        HStack(spacing: 8) {
                            ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317), pointSize: compactChips ? 24 : 32)
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Arcane Resin")
                                HStack(spacing: 4) {
                                    tag(query.arcaneResinAuto ? "Auto" : "≥\(query.arcaneResin)")
                                    if query.arcaneResinFilter.includeMageWand { tag("Mage +2") }
                                    if let depth = query.arcaneResinFilter.maximumDepth { tag("F≤\(depth)") }
                                    if query.arcaneResinFilter.uncursed {
                                        Image(systemName: "shield.lefthalf.filled").font(.caption2).foregroundStyle(.mint)
                                    }
                                }
                            }
                        }
                        .font(compactChips ? .caption : .subheadline)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                        .glassEffect(.regular.interactive(), in: .capsule)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Arcane Resin, \(query.arcaneResinAuto ? "Auto, upgrade matched wands to +3" : "at least \(query.arcaneResin)"), \(query.arcaneResinFilter.summary)")
                    .contextMenu {
                        Button("Remove requirement", role: .destructive, action: removeResin)
                    }
                    .draggable(String.self, id: \.self, item: "arcane_resin")
                    .dragConfiguration(.init(operationsOutsideApp: .init(allowCopy: false)))
                    .onDragSessionUpdated { session in
                        switch session.phase {
                        case .initial, .active: draggingResin = true; draggingKey = nil
                        case .ended(.cancel), .ended(.forbidden), .dataTransferCompleted: draggingResin = false
                        default: break
                        }
                    }
                }
                Button {
                    editor = RequirementsEditorPresentation(blanket: blanket)
                } label: {
                    Label("Add", systemImage: "plus")
                        .font(.subheadline.weight(.semibold))
                        .padding(.horizontal, 16).padding(.vertical, compactChips ? 9 : 13)
                        .glassEffect(.regular.interactive(), in: .capsule)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Add requirement")
            }
            if draggingKey != nil || (!blanket && draggingResin) {
                Label("Drop to remove", systemImage: "trash")
                    .font(.subheadline).foregroundStyle(.red)
                    .frame(maxWidth: .infinity).padding(14)
                    .background(.red.opacity(0.08), in: RoundedRectangle(cornerRadius: 16))
                    .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(.red.opacity(0.4), style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
                    .dropDestination(for: String.self) { (keys: [String], _: CGPoint) -> Bool in
                        if draggingResin && keys.first == "arcane_resin" {
                            removeResin()
                            draggingResin = false
                            return true
                        }
                        guard let key = validDrag(keys) else { return false }
                        remove(key: key)
                        draggingKey = nil
                        return true
                    }
                // Space outside an alternative capsule is the detach target,
                // matching Android's release-on-empty-board gesture.
                Color.clear.frame(height: 28).contentShape(Rectangle())
                    .dropDestination(for: String.self) { (keys: [String], _: CGPoint) -> Bool in detach(keys) }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
        .dropDestination(for: String.self) { (keys: [String], _: CGPoint) -> Bool in detach(keys) }
    }

    @ViewBuilder private func entry(_ item: BoardItem) -> some View {
        if item.cluster == nil {
            chip(requirements[item.anchor], item: item)
        } else {
            RequirementsFlowLayout(spacing: 6) {
                ForEach(Array(item.members.enumerated()), id: \.element) { position, index in
                    HStack(spacing: 5) {
                        if position > 0 { Text("or").font(.caption.monospaced().bold()).foregroundStyle(.purple) }
                        chip(requirements[index], item: item)
                    }
                }
                if requirements.canStack(item) {
                    Button {
                        stackKey = RequirementsStackPresentation(id: requirements[item.anchor].key)
                    } label: { tag("×\(item.stackCount)").padding(.vertical, 10) }
                    .buttonStyle(.plain)
                    .accessibilityLabel("How many")
                }
            }
            .padding(6)
            .background(.purple.opacity(0.055), in: RoundedRectangle(cornerRadius: 27))
            .overlay(RoundedRectangle(cornerRadius: 27).strokeBorder(.purple.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: [5, 3])))
            .dropDestination(for: String.self) { (keys: [String], _: CGPoint) -> Bool in join(keys, target: requirements[item.anchor].key) }
        }
    }

    private func chip(_ requirement: ItemRequirement, item: BoardItem) -> some View {
        Button {
            editor = RequirementsEditorPresentation(key: requirement.key, blanket: requirement.blanket,
                                                   count: item.stackCount, total: item.total,
                                                   copyDepth: requirements.copyDepth(of: item))
        } label: {
            HStack(spacing: compactChips ? 5 : 8) {
                RequirementsSprite(requirement: requirement, size: compactChips ? 24 : 32)
                VStack(alignment: .leading, spacing: 3) {
                    Text(requirement.item?.name ?? "Any \(requirement.kind.singularLabel)")
                        .font(compactChips ? .caption.weight(.medium) : .subheadline.weight(.medium))
                        .lineLimit(2)
                    if !tags(for: requirement).isEmpty {
                        HStack(spacing: 4) {
                            ForEach(tags(for: requirement), id: \.self) { value in tag(value) }
                            if requirement.requireUncursed { Image(systemName: "shield.lefthalf.filled").font(.caption2).foregroundStyle(.mint) }
                        }
                    } else if requirement.requireUncursed {
                        Image(systemName: "shield.lefthalf.filled").font(.caption2).foregroundStyle(.mint)
                    }
                }
                if item.cluster == nil && item.stackCount > 1 { tag(item.total == nil ? "×\(item.stackCount)" : "≤\(item.stackCount)") }
                if let total = item.total, item.cluster == nil { tag("Σ≥\(total)") }
                if requirement.effect == .anyEnchantment {
                    Circle().fill(.purple).frame(width: 7, height: 7)
                } else if requirement.effect.names.count > 1 {
                    tag("\(requirement.effect.names.count)")
                }
            }
            .padding(.leading, 6).padding(.trailing, 11).padding(.vertical, 6)
            .glassEffect(.regular.interactive(), in: .capsule)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(requirement.title + ", " + requirement.description)
        .contextMenu {
            Button(requirement.alternativeGroup == nil ? "Remove requirement" : "Remove alternative", role: .destructive) {
                remove(key: requirement.key)
            }
        }
        .draggable(String.self, id: \.self, item: String(requirement.key))
        .dragConfiguration(.init(operationsOutsideApp: .init(allowCopy: false)))
        .onDragSessionUpdated { session in
            switch session.phase {
            case .initial, .active: draggingKey = requirement.key; draggingResin = false
            case .ended(.cancel), .ended(.forbidden), .dataTransferCompleted: draggingKey = nil
            default: break
            }
        }
        .dropDestination(for: String.self) { (keys: [String], _: CGPoint) -> Bool in join(keys, target: requirement.key) }
    }

    private func tag(_ text: String) -> some View {
        Text(text).font(.caption2.monospaced().weight(.semibold))
            .foregroundStyle(.tint).padding(.horizontal, 5).padding(.vertical, 2)
            .background(.tint.opacity(0.12), in: Capsule())
    }

    private func tags(for requirement: ItemRequirement) -> [String] {
        var values: [String] = []
        switch requirement.tierMatch {
        case .any: break
        case .exactly: values.append("T\(requirement.tier)")
        case .atLeast: values.append("T\(requirement.tier)+")
        case .atMost: values.append("T≤\(requirement.tier)")
        }
        switch requirement.upgradeMatch {
        case .any: break
        case .exactly: values.append("+\(requirement.upgrade)")
        case .atLeast: values.append("+\(requirement.upgrade)↑")
        }
        if requirement.excludeResin { values.append("No resin") }
        if requirement.trinketTransmutations > 0 { values.append("Transmute ≤\(requirement.trinketTransmutations)") }
        if let floor = requirement.maximumDepth { values.append("F≤\(floor)") }
        return values
    }

    private func validDrag(_ keys: [String]) -> Int64? {
        guard let value = keys.first, let key = Int64(value), key == draggingKey,
              requirements.contains(where: { $0.key == key }) else { return nil }
        return key
    }

    private func join(_ keys: [String], target: Int64) -> Bool {
        guard let key = validDrag(keys), let source = requirements.firstIndex(where: { $0.key == key }),
              let target = requirements.firstIndex(where: { $0.key == target }),
              requirements[source].blanket == requirements[target].blanket else { return false }
        query.requirements = requirements.joinAlternatives(source: source, target: target)
        draggingKey = nil
        return true
    }

    private func detach(_ keys: [String]) -> Bool {
        if draggingResin && keys.first == "arcane_resin" { draggingResin = false; return true }
        guard let key = validDrag(keys), let index = requirements.firstIndex(where: { $0.key == key }) else { return false }
        query.requirements = requirements.detach(index)
        draggingKey = nil
        return true
    }

    private func remove(key: Int64) {
        guard let index = requirements.firstIndex(where: { $0.key == key }),
              let item = requirements.boardItem(holding: index) else { return }
        query.requirements = item.cluster == nil ? requirements.removeItem(item) : requirements.removeMember(index)
    }

    private func removeResin() {
        query.arcaneResin = 0
        query.arcaneResinAuto = false
        query.arcaneResinFilter = ArcaneResinFilter()
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
                ItemSpriteView(item: item, glow: requirement.effect.glowName.flatMap { enchantmentGlows[$0] ?? curseGlow }, pointSize: size)
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

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        arrange(subviews, width: proposal.width ?? 360).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let result = arrange(subviews, width: bounds.width)
        for (index, frame) in result.frames.enumerated() {
            subviews[index].place(at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                                  proposal: ProposedViewSize(frame.size))
        }
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> (frames: [CGRect], size: CGSize) {
        var x: CGFloat = 0
        var y: CGFloat = 0
        var rowHeight: CGFloat = 0
        var usedWidth: CGFloat = 0
        var frames: [CGRect] = []
        for subview in subviews {
            let size = subview.sizeThatFits(ProposedViewSize(width: max(1, width), height: nil))
            let childWidth = min(size.width, width)
            if x > 0 && x + childWidth > width {
                x = 0
                y += rowHeight + spacing
                rowHeight = 0
            }
            frames.append(CGRect(x: x, y: y, width: childWidth, height: size.height))
            usedWidth = max(usedWidth, x + childWidth)
            x += childWidth + spacing
            rowHeight = max(rowHeight, size.height)
        }
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
