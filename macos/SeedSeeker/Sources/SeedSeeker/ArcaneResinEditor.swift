import SwiftUI
import SeedSeekerKit

let arcaneResinItem = CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317)

enum RequirementChipDrag: Equatable {
    case item(Int64)
    case resin
}

/// Resin-specific fields inside the requirement editor's existing form.
struct ArcaneResinFields: View {
    @Binding var amount: Int
    @Binding var filter: ArcaneResinFilter

    var body: some View {
        Section {
            Stepper(value: $amount, in: 1...65535) {
                LabeledContent("Minimum resin") {
                    Text("\(amount)").monospacedDigit().foregroundStyle(.secondary)
                }
            }
            Toggle("Require uncursed wands", isOn: $filter.uncursed)
                .toggleStyle(.checkbox)
            Picker("Wand floor limit", selection: $filter.maximumDepth) {
                Text("Search limit").tag(Int?.none)
                ForEach(1...SearchLimits.maxDepth, id: \.self) { depth in Text("Floor \(depth)").tag(Int?.some(depth)) }
            }
            Picker("Wand source", selection: $filter.source) {
                Text("Any source").tag(ScoutItemSource?.none)
                ForEach(ScoutItemSource.allCases, id: \.self) { source in Text(source.label).tag(ScoutItemSource?.some(source)) }
            }
        }
    }
}

/// A query-wide requirement: it supports the board's edit and removal gestures,
/// while item-only relationships (alternatives and stacks) do not apply.
struct ArcaneResinChip: View {
    let amount: Int
    let filter: ArcaneResinFilter
    @Binding var dragging: RequirementChipDrag?
    let onEdit: () -> Void
    let onRemove: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 5) {
            ItemSpriteView(item: arcaneResinItem, pointSize: 16)
            Text(arcaneResinItem.name)
                .font(.system(size: 12, weight: .semibold)).lineLimit(1)
            tag("≥\(amount)", color: .shatteredGreen)
            if let depth = filter.maximumDepth { tag("F≤\(depth)", color: .shatteredYellow) }
            if filter.uncursed { tag("✓", color: .shatteredMint) }
        }
        .padding(.horizontal, 7)
        .frame(height: 30)
        .background(Color(nsColor: .controlBackgroundColor), in: Capsule())
        .overlay(Capsule().strokeBorder(focused ? Color.accentColor : .secondary.opacity(0.35),
                                      lineWidth: focused ? 2 : 1))
        .opacity(dragging == .resin ? 0.35 : 1)
        .contentShape(Capsule())
        .onTapGesture(perform: onEdit)
        .help("≥\(amount) Arcane Resin\n\(filter.summary)")
        .focusable()
        .focused($focused)
        .onKeyPress(.delete) { onRemove(); return .handled }
        .onKeyPress(.return) { onEdit(); return .handled }
        .onDrag {
            dragging = .resin
            return NSItemProvider(object: NSString(string: arcaneResinItem.id))
        }
        .contextMenu {
            Button("Edit…", action: onEdit)
            Divider()
            Button("Remove", role: .destructive, action: onRemove)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("≥\(amount) Arcane Resin, \(filter.summary)")
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { onEdit() }
    }

    private func tag(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold, design: .monospaced))
            .foregroundStyle(color)
            .padding(.horizontal, 4)
            .background(color.opacity(0.13), in: RoundedRectangle(cornerRadius: 4))
    }
}
