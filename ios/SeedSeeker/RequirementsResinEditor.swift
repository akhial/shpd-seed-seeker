import SwiftUI
import SeedSeekerKit

struct RequirementsResinEditor: View {
    let initialAmount: Int
    let initialAuto: Bool
    let initialFilter: ArcaneResinFilter
    let hasRequirement: Bool
    let onSave: (Int, Bool, ArcaneResinFilter) -> Void
    let onRemove: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var minimum: String
    @State private var automatic: Bool
    @State private var filter: ArcaneResinFilter

    init(initialAmount: Int, initialAuto: Bool, initialFilter: ArcaneResinFilter,
         hasRequirement: Bool, onSave: @escaping (Int, Bool, ArcaneResinFilter) -> Void,
         onRemove: @escaping () -> Void) {
        self.initialAmount = initialAmount
        self.initialAuto = initialAuto
        self.initialFilter = initialFilter
        self.hasRequirement = hasRequirement
        self.onSave = onSave
        self.onRemove = onRemove
        _minimum = State(initialValue: String(initialAmount > 0 ? initialAmount : 2))
        _automatic = State(initialValue: initialAuto)
        _filter = State(initialValue: initialFilter)
    }

    private var parsedMinimum: Int? {
        guard let amount = Int(minimum), (1...65_535).contains(amount) else { return nil }
        return amount
    }

    private var floorDescription: String {
        filter.maximumDepth.map { "Wands within floor \($0)" } ?? "Wands within search limit"
    }

    private var floorSelection: Binding<Double> {
        Binding {
            Double(filter.maximumDepth.map { FloorLimits.index(of: $0) + 1 } ?? 0)
        } set: { value in
            let index = min(FloorLimits.options.count, max(0, Int(value.rounded())))
            filter.maximumDepth = index == 0 ? nil : FloorLimits.options[index - 1]
        }
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(spacing: 12) {
                    ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin",
                                                    kind: .wand, spriteIndex: 317), pointSize: 40)
                    Text("Arcane Resin")
                        .font(.title2.bold())
                    Spacer(minLength: 0)
                    Button("Close") { dismiss() }
                        .buttonStyle(.glass)
                }

                Picker("Minimum resin", selection: $automatic) {
                    Text("Amount").tag(false)
                    Text("Auto").tag(true)
                }
                .pickerStyle(.segmented)

                if automatic {
                    Text("Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.")
                        .foregroundStyle(.secondary)
                } else {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Minimum resin")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                        TextField("Minimum resin", text: $minimum)
                            .keyboardType(.numberPad)
                            .textFieldStyle(.roundedBorder)
                            .accessibilityLabel("Minimum resin")
                        if parsedMinimum == nil {
                            Text("Enter a whole number.")
                                .font(.caption)
                                .foregroundStyle(.red)
                        }
                    }
                }

                Toggle("Require uncursed wands", isOn: $filter.uncursed)

                VStack(alignment: .leading, spacing: 8) {
                    Text(floorDescription)
                    Slider(value: floorSelection, in: 0...Double(FloorLimits.options.count), step: 1)
                        .accessibilityLabel(floorDescription)
                }

                Picker("Wand source", selection: $filter.source) {
                    Text("Any source").tag(ScoutItemSource?.none)
                    ForEach(ScoutItemSource.allCases, id: \.self) { source in
                        Text(source.label).tag(ScoutItemSource?.some(source))
                    }
                }
                .pickerStyle(.menu)
                .frame(maxWidth: .infinity, alignment: .leading)

                Toggle(isOn: $filter.includeMageWand) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Include Mage’s starting wand")
                        Text("Adds 2 resin from Magic Missile. Assumes you recover it with Wand Preservation and dismantle it after imbuing.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .accessibilityLabel("Include Mage’s starting wand")

                HStack(spacing: 12) {
                    if hasRequirement {
                        Button("Remove", role: .destructive) {
                            onRemove()
                            dismiss()
                        }
                        .buttonStyle(.glass)
                    }
                    Button {
                        guard automatic || parsedMinimum != nil else { return }
                        onSave(automatic ? 0 : parsedMinimum!, automatic, filter)
                        dismiss()
                    } label: {
                        Text(hasRequirement ? "Save" : "Add")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.glassProminent)
                    .disabled(!automatic && parsedMinimum == nil)
                }
            }
            .padding(20)
        }
        .presentationDragIndicator(.hidden)
        .interactiveDismissDisabled()
    }
}
