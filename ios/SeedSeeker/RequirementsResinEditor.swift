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
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    HStack(spacing: 14) {
                        ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin",
                                                        kind: .wand, spriteIndex: 317), pointSize: 44)
                        RequirementSegmentedControl(title: "Minimum resin",
                                                    options: [(false, "Amount"), (true, "Auto")], selection: $automatic)
                    }

                    if automatic {
                        Text("Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    } else {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Minimum resin")
                                .font(.subheadline.weight(.semibold))
                            TextField("Minimum resin", text: $minimum)
                                .keyboardType(.numberPad)
                                .font(.body.monospacedDigit())
                                .padding(.horizontal, 16).padding(.vertical, 14)
                                .glassEffect(.regular, in: .rect(cornerRadius: 18))
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
                        Text(floorDescription).font(.subheadline.weight(.semibold))
                        RequirementGraduatedSlider(title: floorDescription, value: floorSelection,
                                                   bounds: 0...Double(FloorLimits.options.count))
                    }

                    RequirementSourceSelector(title: "Wand source", source: $filter.source)

                    Toggle(isOn: $filter.includeMageWand) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Include Mage’s starting wand")
                            Text("Adds 2 resin from Magic Missile. Assumes you recover it with Wand Preservation and dismantle it after imbuing.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                    .accessibilityLabel("Include Mage’s starting wand")
                }
                .padding(20)
            }
            .background(Color(uiColor: .systemGroupedBackground))
            .safeAreaBar(edge: .bottom, spacing: 0) {
                GlassEffectContainer(spacing: 18) {
                    HStack(spacing: 12) {
                        if hasRequirement {
                            Button(role: .destructive) {
                                onRemove()
                                dismiss()
                            } label: {
                                Image(systemName: "trash")
                                    .font(.system(size: 19, weight: .medium))
                                    .foregroundStyle(.red)
                                    .frame(width: 48, height: 48)
                                    .glassEffect(.regular.tint(.red.opacity(0.07)).interactive(), in: .circle)
                            }
                            .buttonStyle(.plain)
                            .tint(.red)
                            .accessibilityLabel("Remove")
                        }
                        Spacer(minLength: 12)
                        Button {
                            guard automatic || parsedMinimum != nil else { return }
                            onSave(automatic ? 0 : parsedMinimum!, automatic, filter)
                            dismiss()
                        } label: {
                            HStack(spacing: 9) {
                                Text(hasRequirement ? "Save" : "Add")
                                Image(systemName: "checkmark").font(.subheadline.weight(.semibold))
                            }
                            .font(.headline)
                            .frame(minHeight: 52)
                            .padding(.horizontal, 23)
                            .foregroundStyle(.primary)
                            .glassEffect(.regular.tint(AppTheme.accent.opacity(0.3)).interactive(), in: .capsule)
                        }
                        .buttonStyle(.plain)
                        .disabled(!automatic && parsedMinimum == nil)
                        .opacity(!automatic && parsedMinimum == nil ? 0.45 : 1)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 12)
                .padding(.bottom, 10)
            }
            .scrollEdgeEffectStyle(.soft, for: .vertical)
            .navigationTitle("Arcane Resin")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
            }
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .interactiveDismissDisabled()
    }
}
