// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct SearchSettingsView: View {
    @Environment(\.dismiss) private var dismiss
    @Binding var query: SavedQuery
    var enabled = true
    var challengesEnabled = true
    @AppStorage(WorkerPersistence.defaultsKey) private var savedWorkers = WorkerPersistence.unset

    private let workerCeiling = EngineInfo.availableWorkers
    private var workerCount: Int {
        WorkerPersistence.resolve(saved: savedWorkers, ceiling: workerCeiling)
    }
    private var blacksmithGuaranteed: Bool {
        query.maximumDepth >= ScoutQuestKind.blacksmith.depthRange.upperBound
    }

    var body: some View {
        NavigationStack {
            Form {
                if !enabled {
                    Section { settingsNotice("Stop the search to change its settings.") }
                }
                Section("Dungeon") {
                    VStack(spacing: 12) {
                        settingsValue("Max floor", value: "\(query.maximumDepth)")
                        Slider(value: Binding(
                            get: { Double(FloorLimits.index(of: query.maximumDepth)) },
                            set: { query.maximumDepth = FloorLimits.options[Int($0.rounded())] }
                        ), in: 0...Double(FloorLimits.options.count - 1), step: 1)
                            .accessibilityLabel("Max floor")
                            .accessibilityValue("Floor \(query.maximumDepth)")
                    }
                    .padding(.vertical, 6)
                    .disabled(!enabled)
                }

                Section("Rooms and feelings") {
                    VStack(alignment: .leading, spacing: 16) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Ring of Wealth farming floors").font(.headline)
                            Text("Dark floor with a garden.")
                                .font(.subheadline).foregroundStyle(.secondary)
                        }
                        GlassEffectContainer(spacing: 10) {
                            HStack(spacing: 10) {
                                ForEach(FloorRequirement.farmingFloors, id: \.self) { depth in
                                    farmingFloorButton(depth)
                                }
                            }
                        }
                    }
                    .padding(.vertical, 6)
                    .disabled(!enabled)

                    ForEach(query.floorRequirements.filter { !$0.isFarming }, id: \.depth) { floor in
                        HStack {
                            Text(floorDescription(floor)).font(.subheadline)
                            Spacer()
                            Button("Remove", role: .destructive) {
                                query.floorRequirements.removeAll { $0.depth == floor.depth }
                            }
                            .disabled(!enabled)
                        }
                    }
                    if let floorProblem { settingsNotice(floorProblem) }
                }

                Section("Quests and items") {
                    SettingsToggle(
                        title: "AutoTrinket",
                        supporting: "Applies a helpful trinket at +3 at the first brewing opportunity. Keeps it only when the match needs it.",
                        isOn: $query.autoApplyTrinket
                    )
                    .disabled(!enabled)

                    Picker("Wandmaker quest", selection: $query.wandmakerQuest) {
                        Text("Any quest").tag(Optional<WandmakerQuest>.none)
                        ForEach(WandmakerQuest.allCases, id: \.rawValue) { quest in
                            Text(quest.label).tag(Optional(quest))
                        }
                    }
                    .pickerStyle(.navigationLink)
                    .disabled(!enabled)

                    SettingsToggle(
                        title: "Blacksmith reachable",
                        supporting: blacksmithGuaranteed
                            ? "Already included at this floor limit."
                            : "Reach the Blacksmith within your floor limit.",
                        isOn: Binding(
                            get: { blacksmithGuaranteed || query.requireBlacksmith },
                            set: { query.requireBlacksmith = $0 }
                        )
                    )
                    .disabled(!enabled || blacksmithGuaranteed)

                    SettingsToggle(
                        title: "Exclude smith rewards",
                        supporting: "Items may not come from the 2,000-favor Smith trade.",
                        isOn: $query.excludeBlacksmithRewards
                    )
                    .disabled(!enabled)
                }

                if workerCeiling > 1 {
                    Section("Performance") {
                        VStack(alignment: .leading, spacing: 12) {
                            settingsValue("Workers", value: "\(workerCount) / \(workerCeiling)")
                            Slider(value: Binding(
                                get: { Double(workerCount) },
                                set: { savedWorkers = WorkerPersistence.clamp(Int($0.rounded()), ceiling: workerCeiling) }
                            ), in: 1...Double(workerCeiling), step: 1)
                                .accessibilityLabel("Workers")
                                .accessibilityValue("\(workerCount) of \(workerCeiling) cores")
                            Text("More workers search faster and use more battery.")
                                .font(.subheadline).foregroundStyle(.secondary)
                        }
                        .padding(.vertical, 6)
                        .disabled(!enabled)
                    }
                }

                Section {
                    Text("Used for both searches and scouting.")
                        .font(.subheadline).foregroundStyle(.secondary)
                    if !challengesEnabled {
                        settingsNotice("Challenges can be changed after the current search or scout stops.")
                    }
                    ForEach(Challenge.allCases, id: \.rawValue) { challenge in
                        SettingsToggle(
                            title: challenge.label,
                            supporting: challenge.changesLevelGeneration
                                ? "Changes level generation" : "No effect on seed content",
                            isOn: Binding(
                                get: { query.challenges & challenge.rawValue != 0 },
                                set: { checked in
                                    if checked { query.challenges |= challenge.rawValue }
                                    else { query.challenges &= ~challenge.rawValue }
                                }
                            )
                        )
                        .disabled(!enabled || !challengesEnabled)
                    }
                } header: {
                    Text("Challenges · \(query.challenges.nonzeroBitCount) on")
                }
            }
            .navigationTitle("Search settings")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    @ViewBuilder
    private func farmingFloorButton(_ depth: Int) -> some View {
        let selected = query.floorRequirements.contains { $0.depth == depth && $0.isFarming }
        Button {
            query.toggleFarmingFloor(depth)
        } label: {
            HStack(spacing: 6) {
                if selected { Image(systemName: "checkmark") }
                Text("\(depth)").monospacedDigit()
            }
            .font(.headline)
            .frame(maxWidth: .infinity, minHeight: 44)
        }
        .buttonStyle(.plain)
        .tint(selected ? .accentColor : .secondary)
        .glassEffect(selected ? .regular.tint(.accentColor.opacity(0.2)).interactive() : .regular.interactive())
        .accessibilityLabel("Floor \(depth)")
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private func settingsValue(_ title: String, value: String) -> some View {
        HStack {
            Text(title).font(.headline)
            Spacer()
            Text(value).font(.title3.weight(.semibold)).monospacedDigit().foregroundStyle(.tint)
        }
    }

    private func settingsNotice(_ message: String) -> some View {
        Text(message).font(.subheadline).foregroundStyle(.orange)
    }

    private var floorProblem: String? {
        if Set(query.floorRequirements.map(\.depth)).count != query.floorRequirements.count {
            return "Each floor can have only one requirement."
        }
        if let floor = query.floorRequirements.first(where: { $0.depth > query.maximumDepth }) {
            return "Floor \(floor.depth) exceeds the floor limit of \(query.maximumDepth)."
        }
        return nil
    }

    private func floorDescription(_ floor: FloorRequirement) -> String {
        let feelingLabels = ["none": "Normal", "chasm": "Chasms", "water": "Water", "grass": "Grass",
                             "dark": "Dark", "large": "Large", "traps": "Traps", "secrets": "Secrets"]
        return (["Floor \(floor.depth)", floor.feeling.flatMap { feelingLabels[$0] },
                 floor.rooms.isEmpty ? nil : floor.rooms.joined(separator: ", "),
                 floor.anyRooms.isEmpty ? nil : floor.anyRooms.joined(separator: " / ")]
            .compactMap { $0 }).joined(separator: " · ").replacingOccurrences(of: "_", with: " ")
    }
}

struct AppSettingsView: View {
    @Environment(\.dismiss) private var dismiss
    @AppStorage("compactChips") private var compactChips = false

    var body: some View {
        NavigationStack {
            Form {
                Section("Appearance") {
                    SettingsToggle(
                        title: "Compact chips",
                        supporting: "Shorter chips with smaller sprites and text, so more of the board fits on a line.",
                        isOn: $compactChips
                    )
                }
            }
            .navigationTitle("App settings")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }
}

private struct SettingsToggle: View {
    let title: String
    let supporting: String
    @Binding var isOn: Bool

    var body: some View {
        Toggle(isOn: $isOn) {
            VStack(alignment: .leading, spacing: 5) {
                Text(title).font(.headline)
                Text(supporting).font(.subheadline).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, 6)
        }
    }
}
