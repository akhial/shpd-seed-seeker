import SwiftUI
import SeedSeekerKit

/// The Arcane Resin sheet: a requirement sheet with the resin picked, opened
/// on the query's resin chip or handed over from a wand's item page. Its
/// controls are the shared core's resin section and donor filter.
struct RequirementsResinEditor: View {
    let hasRequirement: Bool
    /// Saves the sheet; answers the sheet to keep showing when the core
    /// refused the save, its errors saying why.
    let onSave: (RequirementSheet) -> RequirementSheet?
    let onRemove: () -> Void

    @Environment(\.dismiss) private var dismiss
    /// The sheet as the core last answered it.
    @State private var sheet: RequirementSheet
    /// The amount as typed; the core is sent whatever number it holds.
    @State private var minimum: String

    init(sheet: RequirementSheet, hasRequirement: Bool,
         onSave: @escaping (RequirementSheet) -> RequirementSheet?,
         onRemove: @escaping () -> Void) {
        self.hasRequirement = hasRequirement
        self.onSave = onSave
        self.onRemove = onRemove
        _sheet = State(initialValue: sheet)
        _minimum = State(initialValue: sheet.form.resin.amountText)
    }

    private var form: SheetForm { sheet.form }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    HStack(spacing: 14) {
                        ItemSpriteView(item: CatalogItem(id: "arcane_resin", name: "Arcane Resin",
                                                        kind: .wand, spriteIndex: 317), pointSize: 44)
                        RequirementSegmentedControl(title: "Minimum resin",
                                                    options: [(false, "Amount"), (true, "Auto")],
                                                    selection: flag(form.resin.auto) { .resinAuto($0) })
                    }

                    if form.resin.auto {
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
                                .onChange(of: minimum) { _, text in
                                    send(.resinAmount(Double(text.trimmingCharacters(in: .whitespaces))))
                                }
                            ForEach(form.errors, id: \.self) { error in
                                Text(error)
                                    .font(.caption)
                                    .foregroundStyle(.red)
                            }
                        }
                    }

                    if form.uncursed.visible {
                        Toggle(form.uncursed.label, isOn: flag(form.uncursed.value) { .uncursed($0) })
                    }

                    if form.floorLimit.visible {
                        RequirementsFloorControl(control: form.floorLimit,
                                                 enabled: flag(form.floorLimit.enabled) { .floorLimitEnabled($0) },
                                                 floor: number(form.floorLimit.value) { .floorLimit($0) })
                    }

                    if form.source.visible {
                        RequirementSourceSelector(title: "Wand source", options: form.source.options,
                                                  selection: pickOptional(form.source.value) { .source($0) })
                    }

                    Toggle(isOn: flag(form.resin.includeMageWand) { .includeMageWand($0) }) {
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
                            if let refused = onSave(sheet) { sheet = refused } else { dismiss() }
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
                        .disabled(!form.canSave)
                        .opacity(form.canSave ? 1 : 0.45)
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

    // MARK: Changes

    /// Sends one change; the core's answer is the sheet shown next.
    private func send(_ change: SheetChange) {
        if let next = sheet.changing(change) { sheet = next }
    }

    private func flag(_ value: Bool, _ change: @escaping @Sendable (Bool) -> SheetChange) -> Binding<Bool> {
        Binding(get: { value }, set: { next in
            if next != value { send(change(next)) }
        })
    }

    private func number(_ value: Int, _ change: @escaping @Sendable (Int) -> SheetChange) -> Binding<Int> {
        Binding(get: { value }, set: { next in
            if next != value { send(change(next)) }
        })
    }

    private func pickOptional(_ value: String?,
                              _ change: @escaping @Sendable (String?) -> SheetChange) -> Binding<String?> {
        Binding(get: { value }, set: { next in
            if next != value { send(change(next)) }
        })
    }
}
