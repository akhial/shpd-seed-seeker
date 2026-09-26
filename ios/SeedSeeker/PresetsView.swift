// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct PresetsView: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Binding var query: SavedQuery
    @Binding var presets: [QueryPreset]
    @State private var presetName = ""
    @State private var saved = 0
    @FocusState private var naming: Bool
    @Namespace private var saveGlass

    private var trimmedName: String { presetName.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var builtInIDs: Set<UUID> { Set(BuiltInPresets.all.map(\.id)) }
    private var replacesPreset: Bool {
        presets.contains { $0.name.caseInsensitiveCompare(trimmedName) == .orderedSame }
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    ForEach(BuiltInPresets.all) { preset in
                        presetButton(preset)
                    }
                    ForEach(presets.filter { !builtInIDs.contains($0.id) }) { preset in
                        HStack {
                            presetButton(preset)
                            Spacer()
                            Button("Delete", role: .destructive) {
                                withAnimation(AppTheme.glassSpring(reduceMotion)) {
                                    presets.removeAll { $0.id == preset.id }
                                }
                            }
                            .buttonStyle(.borderless)
                        }
                        .swipeActions {
                            Button("Delete", role: .destructive) {
                                presets.removeAll { $0.id == preset.id }
                            }
                        }
                    }
                }
            }
            .safeAreaBar(edge: .bottom) { saveBar }
            .sensoryFeedback(.success, trigger: saved)
            .navigationTitle("Presets")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    /// Naming is the only step, so Save buds off the field once there is a
    /// name and folds back into it after saving.
    private var saveBar: some View {
        GlassEffectContainer(spacing: 10) {
            HStack(spacing: 10) {
                TextField("New preset name", text: $presetName)
                    .submitLabel(.done)
                    .onSubmit(savePreset)
                    .focused($naming)
                    .padding(.horizontal, 20)
                    .frame(minHeight: 52)
                    .contentShape(.capsule)
                    .onTapGesture { naming = true }
                    .glassEffect(.regular.tint(naming ? AppTheme.accent.opacity(0.1) : nil).interactive(), in: .capsule)
                    .glassEffectID("name", in: saveGlass)
                if !trimmedName.isEmpty {
                    Button(action: savePreset) {
                        HStack(spacing: 8) {
                            Text(replacesPreset ? "Update" : "Save")
                                .contentTransition(.interpolate)
                            Image(systemName: "checkmark").font(.subheadline.weight(.semibold))
                        }
                        .font(.headline)
                        .foregroundStyle(.primary)
                        .padding(.horizontal, 20)
                        .frame(minHeight: 52)
                        .contentShape(.capsule)
                        .glassEffect(.regular.tint(AppTheme.accent.opacity(0.3)).interactive(), in: .capsule)
                        .glassEffectID("save", in: saveGlass)
                        .glassEffectTransition(.matchedGeometry)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Save current query")
                    .accessibilityHint(replacesPreset ? "Replaces the preset with this name" : "")
                }
            }
            .animation(AppTheme.glassSpring(reduceMotion), value: trimmedName.isEmpty)
            .animation(AppTheme.glassSpring(reduceMotion), value: replacesPreset)
            .animation(AppTheme.glassSpring(reduceMotion), value: naming)
        }
        .padding(.horizontal, 20)
        .padding(.top, 12)
        .padding(.bottom, 10)
    }

    private func presetButton(_ preset: QueryPreset) -> some View {
        Button {
            query = preset.query
            dismiss()
        } label: {
            Text(preset.name)
                .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                .contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
    }

    private func savePreset() {
        guard !trimmedName.isEmpty else { return }
        withAnimation(AppTheme.glassSpring(reduceMotion)) {
            if let index = presets.firstIndex(where: {
                $0.name.caseInsensitiveCompare(trimmedName) == .orderedSame
            }) {
                presets[index].query = query
            } else {
                presets.append(QueryPreset(name: trimmedName, query: query))
            }
            presetName = ""
        }
        naming = false
        saved += 1
    }
}
