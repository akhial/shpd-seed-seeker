// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct PresetsView: View {
    @Environment(\.dismiss) private var dismiss
    @Binding var query: SavedQuery
    @Binding var presets: [QueryPreset]
    @State private var presetName = ""

    private var trimmedName: String { presetName.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var builtInIDs: Set<UUID> { Set(BuiltInPresets.all.map(\.id)) }

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
                                presets.removeAll { $0.id == preset.id }
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
                Section {
                    TextField("New preset name", text: $presetName)
                        .submitLabel(.done)
                        .onSubmit(savePreset)
                    Button(action: savePreset) {
                        Text("Save current query").frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.glassProminent)
                    .disabled(trimmedName.isEmpty)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets(top: 12, leading: 0, bottom: 4, trailing: 0))
                }
            }
            .navigationTitle("Presets")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
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
        if let index = presets.firstIndex(where: {
            $0.name.caseInsensitiveCompare(trimmedName) == .orderedSame
        }) {
            presets[index].query = query
        } else {
            presets.append(QueryPreset(name: trimmedName, query: query))
        }
        presetName = ""
    }
}
