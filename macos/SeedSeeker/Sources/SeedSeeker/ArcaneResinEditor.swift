import SwiftUI
import SeedSeekerKit

let arcaneResinItem = CatalogItem(id: "arcane_resin", name: "Arcane Resin", kind: .wand, spriteIndex: 317)

struct ArcaneResinEditor: View {
    @Environment(\.dismiss) private var dismiss
    let amount: Int
    let onSave: (Int, ArcaneResinFilter) -> Void
    @State private var minimum: String
    @State private var filter: ArcaneResinFilter

    init(amount: Int, filter: ArcaneResinFilter, onSave: @escaping (Int, ArcaneResinFilter) -> Void) {
        self.amount = amount; self.onSave = onSave
        _minimum = State(initialValue: String(amount > 0 ? amount : 2))
        _filter = State(initialValue: filter)
    }

    private var parsed: Int? { Int(minimum).flatMap { (1...65535).contains($0) ? $0 : nil } }

    var body: some View {
        VStack(spacing: 16) {
            Label { Text("Arcane Resin").font(.headline) } icon: { ItemSpriteIcon(item: arcaneResinItem) }
            Form {
                Text("Surplus wands provide 2 × (upgrade + 1) resin each. Wands needed for other requirements are reserved first.")
                    .foregroundStyle(.secondary)
                TextField("Minimum resin", text: $minimum)
                if parsed == nil { Text("Enter a whole number from 1 through 65535.").foregroundStyle(.red) }
                Toggle("Require uncursed wands", isOn: $filter.uncursed)
                Picker("Wand floor limit", selection: $filter.maximumDepth) {
                    Text("Search limit").tag(Int?.none)
                    ForEach(1...SearchLimits.maxDepth, id: \.self) { depth in Text("Floor \(depth)").tag(Int?.some(depth)) }
                }
                Picker("Wand source", selection: $filter.source) {
                    Text("Any source").tag(ScoutItemSource?.none)
                    ForEach(ScoutItemSource.allCases, id: \.self) { source in Text(source.label).tag(ScoutItemSource?.some(source)) }
                }
            }.formStyle(.grouped)
            HStack {
                if amount > 0 {
                    Button("Remove", role: .destructive) { onSave(0, .init()) }
                }
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button(amount > 0 ? "Save" : "Add") {
                    if let parsed { onSave(parsed, filter) }
                }.disabled(parsed == nil).keyboardShortcut(.defaultAction)
            }
        }.padding(20).frame(width: 460, height: 430)
    }
}
