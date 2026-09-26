// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI
import UIKit

struct ScoutView: View {
    @Bindable var model: ScoutModel
    let query: SavedQuery
    let results: [SeedResult]
    let onSelectResult: (String) -> Void
    @State private var choosingDate = false
    @State private var selectedDate = Date()
    @State private var showingInfo = false
    @State private var openMapDepth: Int?
    @State private var headerProgress: CGFloat = 0
    @State private var inputHeight: CGFloat = 172
    @State private var scrollPosition = ScrollPosition(edge: .top)
    @State private var scrollOffset: CGFloat = 0
    @State private var headerDragOrigin: CGFloat?
    @FocusState private var inputFocused: Bool

    private var resultIndex: Int? {
        results.firstIndex { $0.seed == model.requestedSeed }
    }
    private var floors: [Int] {
        guard let world = model.world else { return [] }
        var depths = Set(world.items.map(\.depth))
        depths.formUnion(world.feelings.keys.filter { EngineInfo.shared.levelMapDepths.contains($0) })
        if let last = depths.max() {
            depths.formUnion((1...max(1, last)).filter { EngineInfo.shared.levelMapDepths.contains($0) })
        }
        return depths.sorted()
    }

    var body: some View {
        VStack(spacing: 8) {
            VStack(spacing: 0) {
                seedInput
                    .fixedSize(horizontal: false, vertical: true)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { inputHeight = $0 }
                    .padding(.bottom, 12)
                    .offset(y: -(inputHeight + 12) * headerProgress)
                    .frame(height: (inputHeight + 12) * (1 - headerProgress), alignment: .top)
                    .clipped()
                if let world = model.world {
                    summary(world)
                    if resultIndex != nil || !world.trinketOrder.isEmpty {
                        navigation(world).padding(.top, 4)
                    }
                }
            }
            .padding(.horizontal, 16)
            .simultaneousGesture(DragGesture(minimumDistance: 10).onChanged { value in
                guard model.world != nil, abs(value.translation.height) > abs(value.translation.width) else { return }
                if headerDragOrigin == nil { headerDragOrigin = scrollOffset }
                scrollPosition.scrollTo(y: max(0, (headerDragOrigin ?? 0) - value.translation.height))
            }.onEnded { _ in headerDragOrigin = nil })

            ScrollView {
                LazyVStack(spacing: 0, pinnedViews: [.sectionHeaders]) {
                    if let world = model.world {
                        ForEach(floors, id: \.self) { depth in
                            Section {
                                floorContent(world, depth: depth)
                            } header: {
                                floorHeading(world, depth: depth)
                            }
                        }
                    } else if !model.loading {
                        emptyState.padding(.top, 20)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 24)
            }
            .id(model.world?.seed)
            .scrollPosition($scrollPosition)
            .scrollDismissesKeyboard(.interactively)
            .onScrollGeometryChange(for: CGFloat.self) { geometry in
                max(0, geometry.contentOffset.y + geometry.contentInsets.top)
            } action: { _, offset in
                scrollOffset = offset
                headerProgress = model.world == nil ? 0 : min(1, offset / 96)
            }
            .simultaneousGesture(DragGesture(minimumDistance: 32).onEnded { value in
                guard openMapDepth == nil, abs(value.translation.width) >= 64,
                      abs(value.translation.width) > abs(value.translation.height) * 1.5 else { return }
                step(value.translation.width < 0 ? 1 : -1)
            })
            .accessibilityIdentifier("scout-floors")
        }
        .background(AppTheme.background)
        .navigationTitle("Scout")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: model.world?.seed) { old, new in
            if old != new {
                headerProgress = 0
                openMapDepth = nil
                scrollPosition = ScrollPosition(edge: .top)
                scrollOffset = 0
                headerDragOrigin = nil
            }
        }
        .onChange(of: model.error) { _, error in
            if error != nil { headerProgress = 0 }
        }
        .sheet(isPresented: $choosingDate) { datePicker }
        .sheet(isPresented: $showingInfo) {
            if let world = model.world, let mappings = world.itemMappings {
                ScoutSeedInfoView(seed: world.seed, mappings: mappings)
            }
        }
    }

    private var seedInput: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .bottom, spacing: 8) {
                VStack(alignment: .leading, spacing: 5) {
                    Text("Seed / date").font(.caption).foregroundStyle(.secondary)
                    TextField("Seed / YYYY-MM-DD", text: Binding(
                        get: { model.input }, set: model.editInput))
                        .font(.system(.title3, design: .monospaced))
                        .textInputAutocapitalization(.characters)
                        .autocorrectionDisabled()
                        .keyboardType(.asciiCapable)
                        .submitLabel(.search)
                        .focused($inputFocused)
                        .onSubmit(scoutInput)
                        .disabled(model.loading)
                        .accessibilityIdentifier("scout-run-field")
                }
                .padding(12)
                .background(AppTheme.background.opacity(0.65), in: RoundedRectangle(cornerRadius: 14))
                .overlay(RoundedRectangle(cornerRadius: 14).strokeBorder(.secondary.opacity(0.35)))

                Button {
                    inputFocused = false
                    selectedDate = DailyRunDate.date(model.input) ?? Date()
                    choosingDate = true
                } label: { Image(systemName: "calendar").frame(width: 36, height: 44) }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Choose daily run date")
                    .accessibilityIdentifier("scout-date-picker")
                    .disabled(model.loading)
                Button("Today") { inputFocused = false; onSelectResult(DailyRunDate.code()) }
                    .font(.subheadline.weight(.medium))
                    .frame(height: 44)
                    .disabled(model.loading)
                    .accessibilityIdentifier("scout-today")
            }
            Button(action: scoutInput) {
                HStack(spacing: 10) {
                    if model.loading { ProgressView().tint(AppTheme.background) }
                    Text(model.loading ? "Generating world…" :
                         (model.input.first?.isNumber == true ? "Scout daily run" : "Scout seed"))
                        .fontWeight(.semibold)
                }
                .frame(maxWidth: .infinity, minHeight: 36)
            }
            .buttonStyle(.glassProminent)
            .disabled(model.loading || !SeedCode.isScoutable(model.input))
            if let error = model.error { Text(error).font(.caption).foregroundStyle(.red) }
        }
        .padding(16)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 24))
        .accessibilityIdentifier("scout-input")
    }

    private func scoutInput() {
        guard !model.loading, SeedCode.isScoutable(model.input) else { return }
        inputFocused = false
        onSelectResult(model.input)
    }

    private var emptyState: some View {
        VStack(spacing: 14) {
            Image(systemName: "mappin.and.ellipse").font(.system(size: 40)).foregroundStyle(AppTheme.accent)
            Text("Enter a seed or tap a search result to list its items through floor 24.")
                .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(24)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 22))
    }

    private func summary(_ world: ScoutWorld) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                Text(world.seed)
                    .font(.system(size: 24 - 4 * headerProgress, weight: .medium, design: .monospaced))
                    .foregroundStyle(AppTheme.seed)
                    .lineLimit(1).minimumScaleFactor(0.75)
                    .textSelection(.enabled)
                    .accessibilityIdentifier("scout-seed")
                Spacer(minLength: 0)
                if headerProgress > 0.5, let matches = model.matches {
                    Image(systemName: matches.matchedRequirements == matches.totalRequirements ? "checkmark" : "info.circle")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(matches.matchedRequirements == matches.totalRequirements ? AppTheme.accent : AppTheme.seed)
                        .frame(width: 28, height: 28)
                        .background(.secondary.opacity(0.12), in: Circle())
                        .accessibilityLabel(matchText(matches))
                }
                if world.itemMappings != nil {
                    Button { showingInfo = true } label: { Image(systemName: "info.circle").frame(width: 28, height: 36) }
                        .accessibilityLabel("Seed information")
                }
                Button("Copy") { UIPasteboard.general.string = world.seed }
                    .font(.subheadline.weight(.medium))
            }
            HStack(spacing: 8) {
                ScoutBadge(text: "\(world.items.count) items")
                ScoutBadge(text: "\(Set(world.items.map(\.depth)).count) floors")
                Spacer(minLength: 0)
                if let matches = model.matches {
                    ScoutBadge(text: matchText(matches),
                               color: matches.matchedRequirements == matches.totalRequirements ? AppTheme.accent : AppTheme.seed)
                    .accessibilityIdentifier("scout-requirements")
                }
            }
            .padding(.top, 8)
            .opacity(1 - headerProgress)
            .frame(height: 34 * (1 - headerProgress), alignment: .top)
            .clipped()
            .accessibilityHidden(headerProgress > 0.9)
        }
        .padding(.horizontal, 16).padding(.vertical, 10)
        .background(AppTheme.raised, in: RoundedRectangle(cornerRadius: 20))
        .accessibilityIdentifier("scout-summary")
    }

    private func navigation(_ world: ScoutWorld) -> some View {
        HStack(spacing: 4) {
            if let index = resultIndex {
                Button { step(-1) } label: { Image(systemName: "chevron.left").frame(width: 30, height: 36) }
                    .disabled(index == 0).accessibilityLabel("Previous result")
                Text("\(index + 1) of \(results.count)").font(.caption)
                Button { step(1) } label: { Image(systemName: "chevron.right").frame(width: 30, height: 36) }
                    .disabled(index == results.count - 1).accessibilityLabel("Next result")
            } else { Text("Trinkets").font(.caption) }
            Spacer(minLength: 4)
            if headerProgress > 0.5 || resultIndex == nil {
                ScoutTrinketShortcuts(world: world, enabled: !model.loading, onSelect: model.selectTrinket)
            } else {
                Text("swipe to browse").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .frame(height: 40)
        .accessibilityIdentifier("scout-navigation")
    }

    private func step(_ delta: Int) {
        guard let current = resultIndex, results.indices.contains(current + delta) else { return }
        onSelectResult(results[current + delta].seed)
    }

    private func matchText(_ matches: ScoutMatches) -> String {
        "\(matches.matchedRequirements) of \(matches.totalRequirements) requirement\(matches.totalRequirements == 1 ? "" : "s")"
    }

    private func floorHeading(_ world: ScoutWorld, depth: Int) -> some View {
        let toggle: (() -> Void)? = EngineInfo.shared.levelMapDepths.contains(depth) ? {
            withAnimation(.snappy(duration: 0.2)) {
                openMapDepth = openMapDepth == depth ? nil : depth
            }
        } : nil
        return ScoutFloorHeading(depth: depth, world: world, mapExpanded: openMapDepth == depth,
                                 onMapToggle: toggle)
            .background(AppTheme.background)
    }

    private func floorContent(_ world: ScoutWorld, depth: Int) -> some View {
        let items = Array(world.items.enumerated()).filter { $0.element.depth == depth }
        let choices = ScoutChoiceStatus(items: world.items, matched: model.matches?.matched ?? [])
        let trinkets = items.filter { $0.element.item.kind == .trinket }
        return VStack(spacing: 8) {
            if openMapDepth == depth {
                LevelMapView(world: world, depth: depth,
                             floors: floors.filter { EngineInfo.shared.levelMapDepths.contains($0) },
                             challenges: model.renderedChallenges,
                             changingTrinket: model.loading, onSelectTrinket: model.selectTrinket)
            }
            if items.isEmpty {
                Text("No notable items on this floor.").font(.caption).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 8)
            }
            if !trinkets.isEmpty {
                ScoutTrinketCard(world: world, choices: trinkets, matches: model.matches,
                                 enabled: !model.loading, onSelect: model.selectTrinket)
            }
            ForEach(items.filter { $0.element.item.kind != .trinket }, id: \.offset) { entry in
                let matched = model.matches?.matched.contains(entry.offset) == true
                ScoutItemCard(item: entry.element, ringGems: world.ringGems, matched: matched,
                              dimmed: choices.isDimmed(entry.element.accessibility, matched: matched))
            }
        }.padding(.bottom, 8)
    }

    private var datePicker: some View {
        NavigationStack {
            DatePicker("Daily run date (UTC)", selection: $selectedDate,
                       in: DailyRunDate.date("1970-01-01")!...DailyRunDate.date("9999-12-31")!,
                       displayedComponents: .date)
                .datePickerStyle(.graphical)
                .environment(\.calendar, DailyRunDate.calendar)
                .environment(\.timeZone, DailyRunDate.calendar.timeZone)
                .padding()
                .navigationTitle("Daily run date (UTC)")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { choosingDate = false }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Use date") {
                            model.editInput(DailyRunDate.code(selectedDate))
                            choosingDate = false
                        }
                    }
                }
        }
        .presentationDetents([.medium, .large])
    }
}

struct ScoutBadge: View {
    let text: String
    var color: Color = .secondary
    var body: some View {
        Text(text).font(.caption2.weight(.medium)).foregroundStyle(color)
            .padding(.horizontal, 7).padding(.vertical, 4)
            .background(color.opacity(0.12), in: RoundedRectangle(cornerRadius: 6))
            .lineLimit(1).minimumScaleFactor(0.7)
    }
}

struct ScoutFloorHeading: View {
    let depth: Int
    let world: ScoutWorld
    var mapExpanded = false
    var onMapToggle: (() -> Void)?
    var onCloseMap: (() -> Void)?
    private var region: String {
        switch depth {
        case ..<6: "Sewers"
        case ..<11: "Prison"
        case ..<16: "Caves"
        case ..<21: "Dwarven City"
        default: "Demon Halls"
        }
    }
    private var color: Color {
        switch depth {
        case ..<6: AppTheme.accent
        case ..<11: .orange
        case ..<16: .cyan
        case ..<21: .purple
        default: .red
        }
    }
    var body: some View {
        Group {
            if let onMapToggle {
                Button(action: onMapToggle) { content }
                    .buttonStyle(.plain)
                    .accessibilityLabel("\(mapExpanded ? "Hide" : "Show") floor \(depth) map")
                    .accessibilityValue(mapExpanded ? "Map expanded" : "Map collapsed")
            } else { content }
        }.frame(minHeight: 48)
    }
    private var content: some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 3, height: 16)
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 7) {
                    Text("FLOOR \(depth)").font(.caption.weight(.bold)).tracking(1.1)
                    if let feeling = world.feelings[depth] { FloorFeelingSpriteView(feeling: feeling) }
                    Text(region).font(.caption).foregroundStyle(color)
                }
                if let quest = world.quests.first(where: { $0.depth == depth }) {
                    ScoutBadge(text: quest.variant.label, color: color)
                }
            }
            Spacer(minLength: 0)
            if world.isFarmingFloor(depth) { ScoutBadge(text: "Garden", color: AppTheme.accent) }
            let count = world.items.filter { $0.depth == depth }.count
            Text(count == 1 ? "1 item" : "\(count) items").font(.caption2).foregroundStyle(.secondary)
            if let onCloseMap {
                Button(action: onCloseMap) { Image(systemName: "xmark").frame(width: 32, height: 32) }
                    .accessibilityLabel("Close map")
            } else if onMapToggle != nil {
                Image(systemName: mapExpanded ? "chevron.down" : "chevron.right")
                    .font(.caption).foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}
