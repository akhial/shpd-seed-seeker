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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Namespace private var sheetZoom
    @Namespace private var inputGlass

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
                        ScoutArtifactDeckView(world: world, matches: model.matches)
                            .padding(.vertical, 4)
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
        .sheet(isPresented: $choosingDate) {
            datePicker.navigationTransition(.zoom(sourceID: "date", in: sheetZoom))
        }
        .sheet(isPresented: $showingInfo) {
            if let world = model.world, let mappings = world.itemMappings {
                ScoutSeedInfoView(seed: world.seed, mappings: mappings)
                    .navigationTransition(.zoom(sourceID: "info", in: sheetZoom))
            }
        }
    }

    /// The field, Today and Scout are one cluster of glass. The field
    /// brightens while it has focus; Scout is the tinted, primary shape.
    private var seedInput: some View {
        let scoutable = SeedCode.isScoutable(model.input)
        // Once its world is showing, the seed has nothing new to reveal.
        let fresh = scoutable && model.world?.seed != model.input
        return GlassEffectContainer(spacing: 10) {
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 10) {
                    HStack(spacing: 6) {
                        VStack(alignment: .leading, spacing: 3) {
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
                        Button {
                            inputFocused = false
                            selectedDate = DailyRunDate.date(model.input) ?? Date()
                            choosingDate = true
                        } label: {
                            Image(systemName: "calendar")
                                .font(.body.weight(.medium))
                                .foregroundStyle(AppTheme.accent)
                                .frame(width: 44, height: 44)
                                .contentShape(.rect)
                        }
                        .buttonStyle(.plain)
                        .matchedTransitionSource(id: "date", in: sheetZoom)
                        .accessibilityLabel("Choose daily run date")
                        .accessibilityIdentifier("scout-date-picker")
                        .disabled(model.loading)
                    }
                    .padding(.leading, 18).padding(.trailing, 6).padding(.vertical, 10)
                    .contentShape(.rect(cornerRadius: 24))
                    .onTapGesture { inputFocused = true }
                    .glassEffect(.regular.tint(inputFocused ? AppTheme.accent.opacity(0.1) : nil),
                                 in: .rect(cornerRadius: 24))
                    .glassEffectID("field", in: inputGlass)

                    Button { inputFocused = false; onSelectResult(DailyRunDate.code()) } label: {
                        Text("Today")
                            .font(.subheadline.weight(.semibold))
                            .foregroundStyle(AppTheme.accent)
                            .padding(.horizontal, 16)
                            .frame(maxHeight: .infinity)
                            .contentShape(.rect(cornerRadius: 24))
                            .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 24))
                            .glassEffectID("today", in: inputGlass)
                    }
                    .buttonStyle(.plain)
                    .disabled(model.loading)
                    .accessibilityIdentifier("scout-today")
                }
                .fixedSize(horizontal: false, vertical: true)

                Button(action: scoutInput) {
                    HStack(spacing: 10) {
                        if model.loading { ProgressView().tint(AppTheme.upgrade) }
                        Text(model.loading ? "Generating world…" :
                             (model.input.first?.isNumber == true ? "Scout daily run" : "Scout seed"))
                            .contentTransition(.interpolate)
                        if !model.loading {
                            Image(systemName: "arrow.right")
                                .font(.subheadline.weight(.bold))
                                .offset(x: scoutable ? 0 : -4)
                                .opacity(scoutable ? 1 : 0)
                        }
                    }
                    .font(.headline)
                    .foregroundStyle(scoutable || model.loading ? Color.primary : Color.secondary)
                    .frame(maxWidth: .infinity, minHeight: 52)
                    .contentShape(.capsule)
                    .glassEffect(.regular.tint(fresh || model.loading ? AppTheme.accent.opacity(0.3) : nil).interactive(),
                                 in: .capsule)
                    .glassEffectID("scout", in: inputGlass)
                }
                .buttonStyle(.plain)
                .disabled(model.loading || !scoutable)
                if let error = model.error { Text(error).font(.caption).foregroundStyle(.red) }
            }
            .animation(AppTheme.glassSpring(reduceMotion), value: scoutable)
            .animation(AppTheme.glassSpring(reduceMotion), value: fresh)
            .animation(AppTheme.glassSpring(reduceMotion), value: model.loading)
            .animation(AppTheme.glassSpring(reduceMotion), value: inputFocused)
        }
        .padding(.top, 4)
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
                    Button { showingInfo = true } label: {
                        Image(systemName: "info")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(Color.primary.opacity(0.8))
                            .frame(width: 36, height: 36)
                            .glassEffect(.regular.interactive(), in: .circle)
                    }
                    .buttonStyle(.plain)
                    .matchedTransitionSource(id: "info", in: sheetZoom)
                    .accessibilityLabel("Seed information")
                }
                GlassCopyButton(text: world.seed, label: "Copy", size: 36)
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
        HStack(spacing: 8) {
            if let index = resultIndex {
                HStack(spacing: 0) {
                    Button { step(-1) } label: {
                        Image(systemName: "chevron.left").frame(width: 38, height: 38).contentShape(.rect)
                    }
                    .disabled(index == 0).opacity(index == 0 ? 0.3 : 1)
                    .accessibilityLabel("Previous result")
                    Text("\(index + 1) of \(results.count)")
                        .font(.caption.weight(.semibold)).monospacedDigit()
                        .contentTransition(.numericText(value: Double(index)))
                        .frame(minWidth: 52)
                    Button { step(1) } label: {
                        Image(systemName: "chevron.right").frame(width: 38, height: 38).contentShape(.rect)
                    }
                    .disabled(index == results.count - 1).opacity(index == results.count - 1 ? 0.3 : 1)
                    .accessibilityLabel("Next result")
                }
                .font(.subheadline.weight(.semibold))
                .buttonStyle(.plain)
                .glassEffect(.regular.interactive(), in: .capsule)
                .animation(reduceMotion ? nil : .snappy, value: index)
            } else { Text("Trinkets").font(.caption) }
            Spacer(minLength: 4)
            if headerProgress > 0.5 || resultIndex == nil {
                TrinketGlassShortcuts(world: world, enabled: !model.loading, size: 36, onSelect: model.selectTrinket)
            } else {
                Text("swipe to browse").font(.caption2).foregroundStyle(.secondary)
            }
        }
        .frame(height: 44)
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
            withAnimation(reduceMotion ? nil : .snappy(duration: 0.3)) {
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
                    .transition(.asymmetric(insertion: .scale(scale: 0.96, anchor: .top).combined(with: .opacity),
                                            removal: .opacity))
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
    var weight: Font.Weight = .medium
    var body: some View {
        Text(text).font(.caption2.weight(weight)).foregroundStyle(color)
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
    private var regionRGB: (red: Double, green: Double, blue: Double) {
        switch depth {
        case ..<6: (127, 226, 184)
        case ..<11: (143, 183, 232)
        case ..<16: (216, 162, 107)
        case ..<21: (201, 166, 232)
        default: (232, 143, 143)
        }
    }
    private var color: Color {
        Color(red: regionRGB.red / 255, green: regionRGB.green / 255, blue: regionRGB.blue / 255)
    }
    private var questColor: Color {
        Color(red: (regionRGB.red * 0.84 + 234 * 0.16) / 255,
              green: (regionRGB.green * 0.84 + 234 * 0.16) / 255,
              blue: (regionRGB.blue * 0.84 + 234 * 0.16) / 255)
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
        HStack(spacing: 6) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 3, height: 16)
            Text("FLOOR \(depth)").font(.caption.weight(.bold)).tracking(0.25).fixedSize()
            if let feeling = world.feelings[depth] { FloorFeelingSpriteView(feeling: feeling).fixedSize() }
            Text(region).font(.caption).foregroundStyle(color.opacity(0.9))
                .lineLimit(1).minimumScaleFactor(0.85)
            if let quest = world.quests.first(where: { $0.depth == depth }) {
                Text(quest.variant.label).font(.caption2.weight(.semibold))
                    .foregroundStyle(questColor)
                    .lineLimit(1).minimumScaleFactor(0.8)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(color.opacity(0.11), in: Capsule())
                    .overlay(Capsule().strokeBorder(color.opacity(0.3), lineWidth: 1))
            }
            Spacer(minLength: 0)
            if world.isFarmingFloor(depth) { ScoutBadge(text: "Garden", color: AppTheme.accent) }
            let count = world.items.filter { $0.depth == depth }.count
            Text(count == 1 ? "1 item" : "\(count) items").font(.caption2).foregroundStyle(.secondary).fixedSize()
            if let onCloseMap {
                Button("Close map", systemImage: "xmark", action: onCloseMap)
                    .labelStyle(.iconOnly)
                    .font(.subheadline.weight(.semibold))
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
            } else if onMapToggle != nil {
                Image(systemName: "chevron.right")
                    .font(.caption).foregroundStyle(.secondary)
                    .rotationEffect(.degrees(mapExpanded ? 90 : 0))
            }
        }
        .padding(.vertical, 6)
        .contentShape(Rectangle())
    }
}
