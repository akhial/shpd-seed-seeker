import SwiftUI
import SeedSeekerKit

let arcaneResinItem = CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317)

enum RequirementChipDrag: Equatable {
    case item(Int64)
    case resin
}

/// A query-wide requirement: it supports the board's edit and removal gestures,
/// while item-only relationships (alternatives and stacks) do not apply. Its
/// words are the shared core's board chip for the query's resin condition.
struct ArcaneResinChip: View {
    let chip: BoardResinChip
    @Binding var dragging: RequirementChipDrag?
    let onEdit: () -> Void
    let onRemove: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 5) {
            ItemSpriteView(item: arcaneResinItem, pointSize: 16)
            Text(chip.name)
                .font(.system(size: 12, weight: .semibold)).lineLimit(1)
            ForEach(chip.tags, id: \.self) { tag in tagView(tag.text, color: .shatteredYellow) }
            if chip.uncursed { tagView("✓", color: .shatteredMint) }
        }
        .padding(.horizontal, 7)
        .frame(height: 30)
        .background(Color(nsColor: .controlBackgroundColor), in: Capsule())
        .overlay(Capsule().strokeBorder(focused ? Color.accentColor : .secondary.opacity(0.35),
                                      lineWidth: focused ? 2 : 1))
        .opacity(dragging == .resin ? 0.35 : 1)
        .contentShape(Capsule())
        .onTapGesture(perform: onEdit)
        .help(helpText)
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
        .accessibilityLabel(chip.description)
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { onEdit() }
    }

    /// The chip's name and details, then what the Auto amount means.
    private var helpText: String {
        var lines = [chip.name]
        if !chip.details.isEmpty { lines.append(chip.details.joined(separator: " · ")) }
        if let amount = chip.amountTooltip { lines.append(amount) }
        return lines.joined(separator: "\n")
    }

    private func tagView(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold, design: .monospaced))
            .foregroundStyle(color)
            .padding(.horizontal, 4)
            .background(color.opacity(0.13), in: RoundedRectangle(cornerRadius: 4))
    }
}
