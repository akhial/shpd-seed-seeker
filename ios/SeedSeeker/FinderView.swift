// SPDX-License-Identifier: GPL-3.0-or-later
import SwiftUI
import SeedSeekerKit
import UIKit

struct FinderView: View {
    @Bindable var model: AppModel
    let sheetZoom: Namespace.ID
    @State private var boardInteraction = RequirementBoardInteraction()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Namespace private var searchGlass

    private var controller: SearchController { model.controller }
    private var requirementCount: Int {
        model.query.requirements.filter { !$0.blanket }.boardCount
            + (model.query.arcaneResinAuto || model.query.arcaneResin > 0 ? 1 : 0)
    }
    private var requirementsSummary: String {
        let requirements = model.query.requirements
        var parts = requirements.boardItems().map { board in
            (requirements[board.anchor].blanket ? "Blanket: " : "")
                + board.members.map { requirements[$0].title }.joined(separator: " or ")
                + (board.stackCount > 1 ? " ×\(board.stackCount)" : "")
        }
        if model.query.arcaneResinAuto { parts.append("Auto Arcane Resin") }
        else if model.query.arcaneResin > 0 { parts.append("≥\(model.query.arcaneResin) Arcane Resin") }
        return parts.joined(separator: " · ")
    }
    private var resultTitle: String {
        if controller.isRunning && controller.refineProgress != nil { return "Results — \(controller.foundCount) · refining" }
        if controller.isRunning && controller.refinedOf != nil { return "Results — \(controller.foundCount) · searching" }
        if controller.isRunning { return "Results — \(controller.foundCount) · live" }
        if controller.state == .completed { return "Results — \(controller.foundCount) found" }
        if controller.state == .cancelled { return "Results — \(controller.foundCount) · cancelled" }
        return "Results"
    }
    private var emptyMessage: String {
        if controller.isRunning { return "0 matches yet." }
        if controller.isImpossibleQuery { return "Impossible query. No seed can satisfy this combination of requirements." }
        if controller.state == .completed { return "0 matches." }
        return "No results — run a search."
    }
    private var scopeSummary: String {
        var parts = ["≤ floor \(model.query.maximumDepth)"]
        if let quest = model.query.wandmakerQuest { parts.append(quest.label) }
        if model.query.requireBlacksmith { parts.append("smith") }
        if model.query.excludeBlacksmithRewards { parts.append("no smith rewards") }
        let count = model.query.challenges.nonzeroBitCount
        if count > 0 { parts.append("\(count) challenge\(count == 1 ? "" : "s")") }
        if !model.query.autoApplyTrinket { parts.append("AutoTrinket off") }
        if !model.query.floorRequirements.isEmpty {
            parts.append("Required floors: " + model.query.floorRequirements.map { "\($0.depth)" }.joined(separator: ", "))
        }
        return parts.joined(separator: " · ")
    }

    var body: some View {
        VStack(spacing: 0) {
            pageHeader("Requirements (\(requirementCount))", open: !model.showResults,
                       actionLabel: "Show requirements", summary: requirementsSummary) { showResults(false) }
            if !model.showResults { queryPage.transition(.opacity) }
            Divider()
            pageHeader(resultTitle, open: model.showResults,
                       actionLabel: "Show results") { showResults(true) }
            if model.showResults { resultsPage.transition(.opacity) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(AppTheme.background)
        .safeAreaInset(edge: .bottom, spacing: 0) { searchBar }
        .overlay { RequirementsLiftOverlay(interaction: boardInteraction) }
        .sensoryFeedback(.impact(weight: .medium), trigger: controller.isRunning) { _, running in running }
        .sensoryFeedback(.success, trigger: controller.state) { _, state in state == .completed }
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button("Presets") { model.sheet = .presets }.disabled(controller.isRunning)
            }
            .matchedTransitionSource(id: "presets", in: sheetZoom)
            ToolbarItem(placement: .topBarTrailing) {
                Button("Settings", systemImage: "gearshape") { model.sheet = .settings }
            }
            .matchedTransitionSource(id: "settings-finder", in: sheetZoom)
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button("Share search…", systemImage: "square.and.arrow.up", action: model.share)
                        .disabled(model.query.slotCount == 0)
                    Button("Import results…", systemImage: "square.and.arrow.down") { model.showingImporter = true }
                        .disabled(controller.isRunning)
                    Button("Import from clipboard", systemImage: "doc.on.clipboard") { model.sheet = .clipboardImport }
                        .disabled(controller.isRunning)
                    Button("Export results…", systemImage: "doc.badge.arrow.up", action: model.export)
                        .disabled(controller.isRunning || controller.results.isEmpty)
                    Button("Clear results", systemImage: "trash") {
                        controller.clearResults(); model.importNotice = nil; model.showResults = false
                    }.disabled(!controller.canClearResults)
                    Divider()
                    Button("About and licenses", systemImage: "info.circle") { model.sheet = .about }
                } label: { Label("More options", systemImage: "ellipsis") }
            }
            .matchedTransitionSource(id: "more-finder", in: sheetZoom)
        }
    }

    private func showResults(_ show: Bool) {
        withAnimation(reduceMotion ? nil : .snappy(duration: 0.32)) { model.showResults = show }
    }

    private func pageHeader(_ title: String, open: Bool, actionLabel: String,
                            summary: String? = nil,
                            action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Text(title).font(.subheadline.weight(.semibold))
                    .contentTransition(.numericText())
                    .layoutPriority(1)
                if !open, let summary, !summary.isEmpty {
                    Text(summary).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
                Spacer(minLength: 8)
                Image(systemName: "chevron.down").font(.caption.weight(.semibold))
                    .opacity(open ? 0 : 1)
                    .rotationEffect(.degrees(open ? -180 : 0))
            }
            .foregroundStyle(open ? Color.primary : Color.secondary)
            .padding(.horizontal, 20).padding(.vertical, 14)
            .contentShape(Rectangle())
            .animation(reduceMotion ? nil : .snappy, value: title)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(open ? title : "\(actionLabel), \(title)")
    }

    private var queryPage: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                RequirementsView(query: $model.query, interaction: boardInteraction)
                    .disabled(controller.isRunning)
                if let message = model.validationMessage, model.query.slotCount > 0 {
                    Text(message).font(.footnote).foregroundStyle(.red)
                }
                Button { model.sheet = .searchSettings } label: {
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 9) {
                            searchSettingsIcon
                            Text("Search settings").font(.subheadline.weight(.medium)).foregroundStyle(.primary)
                            Text(scopeSummary).font(.caption).foregroundStyle(.secondary)
                            searchSettingsChevron
                        }
                        .fixedSize(horizontal: true, vertical: false)
                        HStack(spacing: 9) {
                            searchSettingsIcon
                            VStack(alignment: .leading, spacing: 3) {
                                Text("Search settings").font(.subheadline.weight(.medium)).foregroundStyle(.primary)
                                Text(scopeSummary).font(.caption).foregroundStyle(.secondary)
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            searchSettingsChevron
                        }
                    }
                    .padding(.horizontal, 14).padding(.vertical, 10)
                    .frame(minHeight: 46)
                    .contentShape(RoundedRectangle(cornerRadius: 23))
                    .glassEffect(.regular.tint(.white.opacity(0.02)).interactive(), in: .rect(cornerRadius: 23))
                }
                .buttonStyle(.plain)
                .matchedTransitionSource(id: "search-settings", in: sheetZoom)
            }
            .frame(maxWidth: 680, alignment: .leading)
            .padding(.horizontal, 16).padding(.bottom, 28)
            .frame(maxWidth: .infinity)
        }
        .scrollDisabled(boardInteraction.isDragging)
        .scrollClipDisabled()
        .scrollEdgeEffectStyle(.soft, for: .vertical)
    }

    private var searchSettingsIcon: some View {
        Image(systemName: "slider.horizontal.3")
            .font(.system(size: 18, weight: .medium))
            .foregroundStyle(.secondary)
    }

    private var searchSettingsChevron: some View {
        Image(systemName: "chevron.right")
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(.tertiary)
    }

    private var resultsPage: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 8) {
                if let notice = model.importNotice {
                    Text(notice).font(.footnote).foregroundStyle(.secondary).padding(.vertical, 4)
                }
                if let message = controller.message {
                    Text(message).font(.footnote).foregroundStyle(.red)
                }
                if controller.results.isEmpty {
                    emptyResults
                }
                ForEach(controller.results, id: \.seed) { result in
                    resultRow(result)
                }
            }.frame(maxWidth: 680, alignment: .leading)
                .padding(.horizontal, 16).padding(.bottom, 12).frame(maxWidth: .infinity, alignment: .center)
        }
    }

    private var emptyResults: some View {
        VStack(spacing: 12) {
            Image(systemName: controller.isImpossibleQuery ? "exclamationmark.magnifyingglass" : "sparkle.magnifyingglass")
                .font(.system(size: 34, weight: .medium))
                .foregroundStyle(controller.isRunning ? AppTheme.accent : .secondary)
                .symbolEffect(.breathe, options: .repeating, isActive: controller.isRunning && !reduceMotion)
            Text(emptyMessage).font(.subheadline).foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 36)
    }

    /// The whole row scouts its seed; only the glass copy control stays apart.
    /// The last scouted seed keeps a faint outline, so returning finds your place.
    private func resultRow(_ result: SeedResult) -> some View {
        let scouted = controller.selectedSeed == result.seed
        return HStack(spacing: 10) {
            Button { model.scoutResult(result.seed) } label: {
                HStack(spacing: 10) {
                    Text(result.seed).font(.system(.headline, design: .monospaced)).tracking(1)
                        .foregroundStyle(AppTheme.seed)
                    if let id = result.selectedTrinket, let item = ItemCatalog.findById(id) {
                        ItemSpriteView(item: item, pointSize: 20).opacity(0.7)
                    }
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
                }
                .frame(minHeight: 58).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(result.seed)
            .accessibilityHint("Scout seed")
            GlassCopyButton(text: result.seed, label: "Copy \(result.seed)", size: 40)
        }
        .padding(.leading, 16).padding(.trailing, 9)
        .background(AppTheme.raised, in: RoundedRectangle(cornerRadius: 18))
        .overlay {
            RoundedRectangle(cornerRadius: 18)
                .strokeBorder(AppTheme.accent.opacity(scouted ? 0.45 : 0), lineWidth: 1)
        }
    }

    /// Search grows into the live progress lens and the stop control buds off
    /// its trailing edge. A lifted chip turns the same glass into the drop target.
    private var searchBar: some View {
        GlassEffectContainer(spacing: 14) {
            HStack(spacing: 10) {
                if boardInteraction.isDragging {
                    RequirementsRemoveTarget(interaction: boardInteraction)
                        .glassEffectID("action", in: searchGlass)
                        .glassEffectTransition(.matchedGeometry)
                } else if controller.isRunning {
                    progressLens
                        .glassEffectID("action", in: searchGlass)
                        .glassEffectTransition(.matchedGeometry)
                    Button { model.backgroundSearch.stop() } label: {
                        Image(systemName: "stop.fill")
                            .font(.system(size: 18, weight: .semibold))
                            .foregroundStyle(.red)
                            .frame(width: 54, height: 54)
                            .contentShape(.circle)
                            .glassEffect(.regular.tint(.red.opacity(0.08)).interactive(), in: .circle)
                            .glassEffectID("stop", in: searchGlass)
                            .glassEffectTransition(.matchedGeometry)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Cancel")
                } else {
                    Button {
                        withAnimation(reduceMotion ? nil : .snappy(duration: 0.32)) { model.search() }
                    } label: {
                        Label("Search", systemImage: "magnifyingglass")
                            .font(.headline)
                            .foregroundStyle(.primary)
                            .frame(minWidth: 112, minHeight: 54)
                            .padding(.horizontal, 24)
                            .contentShape(Capsule())
                            .glassEffect(.regular.tint(AppTheme.accent.opacity(0.28)).interactive(), in: .capsule)
                            .glassEffectID("action", in: searchGlass)
                            .glassEffectTransition(.matchedGeometry)
                    }
                    .buttonStyle(.plain)
                    .disabled(model.request == nil)
                    .opacity(model.request == nil ? 0.45 : 1)
                }
            }
            .frame(maxWidth: 680)
            .animation(reduceMotion ? nil : .spring(response: 0.46, dampingFraction: 0.8), value: controller.isRunning)
            .animation(reduceMotion ? nil : .spring(response: 0.38, dampingFraction: 0.76), value: boardInteraction.isDragging)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16).padding(.top, 12).padding(.bottom, 16)
    }

    /// Each find lands in the count with a small pulse while light sweeps
    /// through the glass for as long as the engine is working.
    private var progressLens: some View {
        HStack(spacing: 12) {
            ZStack {
                if controller.isPreparing {
                    ProgressView().controlSize(.small).tint(AppTheme.seed)
                } else {
                    Text(compactCount(Int64(controller.foundCount)))
                        .font(.subheadline.weight(.bold))
                        .foregroundStyle(AppTheme.seed)
                        .contentTransition(.numericText(value: Double(controller.foundCount)))
                        .lineLimit(1).minimumScaleFactor(0.6)
                }
            }
            .padding(.horizontal, 6)
            .frame(minWidth: 36, minHeight: 36)
            .background(AppTheme.seed.opacity(0.13), in: .capsule)
            .keyframeAnimator(initialValue: 1.0, trigger: reduceMotion ? 0 : controller.foundCount) { view, scale in
                view.scaleEffect(scale)
            } keyframes: { _ in
                SpringKeyframe(1.2, duration: 0.12, spring: .snappy)
                SpringKeyframe(1, duration: 0.35, spring: .bouncy)
            }
            .accessibilityLabel("\(controller.foundCount) found")
            VStack(alignment: .leading, spacing: 3) {
                Text(progressText).font(.caption.weight(.semibold))
                Text(estimateText).font(.caption2).foregroundStyle(.secondary)
            }
            .monospacedDigit()
            .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1).minimumScaleFactor(0.8)
            .fixedSize(horizontal: false, vertical: dynamicTypeSize.isAccessibilitySize)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, 9).padding(.trailing, 16).padding(.vertical, 8)
        .frame(minHeight: 54)
        .background { GlassSheen(shape: Capsule()) }
        .glassEffect(.regular.tint(AppTheme.accent.opacity(0.1)), in: .capsule)
        .animation(reduceMotion ? nil : .snappy, value: controller.foundCount)
        .accessibilityElement(children: .combine)
    }

    private var estimateText: String {
        if let progress = controller.refineProgress { return "\(progress.checked) of \(progress.total) checked" }
        guard controller.matchProbability != nil, controller.seedsPerSecond > 0 else { return "p estimating… · est —" }
        return "p \(controller.probabilityLabel) · est \(NumberFormat.estimateDuration(controller.timeToSeed))"
    }

    private var progressText: String {
        if controller.isPreparing { return "Preparing search…" }
        if controller.refineProgress != nil { return "Checking saved seeds · \(NumberFormat.duration(controller.elapsed))" }
        return "\(NumberFormat.seedRate(controller.seedsPerSecond)) seeds/s · \(NumberFormat.duration(controller.elapsed)) · \(compactCount(controller.scannedSeeds)) scanned"
    }
}

func compactCount(_ value: Int64) -> String {
    for (scale, format, suffix): (Double, String, String) in [
        (1e12, "%.2f", "T"), (1e9, "%.2f", "B"), (1e6, "%.1f", "M"), (1e3, "%.1f", "K")
    ] where Double(value) >= scale {
        return String(format: format, Double(value) / scale) + suffix
    }
    return String(value)
}
