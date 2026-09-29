import SwiftUI
import SeedSeekerKit

/// The requirement sheet. Every control is drawn from the shared core's
/// form — whether it shows, what it offers, its bounds, its words and its
/// help text — and each move is one change sent to the core, whose answer is
/// the form drawn next. The sheet keeps what is its own: the two pages, the
/// glass, the title and buttons, and the headings of its pickers.
struct RequirementsEditor: View {
    @Environment(\.dismiss) private var dismiss
    /// Hands the sheet over to the resin sheet once Arcane Resin is picked.
    let onPickResin: (RequirementSheet) -> Void
    /// Saves the sheet onto the board; answers the sheet to keep showing
    /// when the core refused the save, its errors saying why.
    let onSave: (RequirementSheet) -> RequirementSheet?
    let onRemove: (() -> Void)?

    /// The sheet as the core last answered it.
    @State private var sheet: RequirementSheet
    @State private var details: Bool
    @Namespace private var editorGlass
    @Namespace private var selectorGlass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(sheet: RequirementSheet,
         onPickResin: @escaping (RequirementSheet) -> Void,
         onSave: @escaping (RequirementSheet) -> RequirementSheet?,
         onRemove: (() -> Void)? = nil) {
        self.onPickResin = onPickResin
        self.onSave = onSave
        self.onRemove = onRemove
        _sheet = State(initialValue: sheet)
        _details = State(initialValue: sheet.form.mode == .edit)
    }

    private var form: SheetForm { sheet.form }

    private var title: String {
        if form.blanket { return form.mode == .new ? "Add blanket requirement" : "Edit blanket requirement" }
        if form.mode == .new { return "Add requirement" }
        return form.inCluster ? "Edit alternative" : "Edit requirement"
    }

    var body: some View {
        NavigationStack {
            Group {
                if details { detailsPage } else { itemPage }
            }
            .safeAreaBar(edge: .bottom, spacing: 0) { footer }
            .scrollEdgeEffectStyle(.soft, for: .vertical)
            .background(Color(uiColor: .systemGroupedBackground))
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Close") { dismiss() } }
            }
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
    }

    private var itemPage: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 100), spacing: 10)], spacing: 10) {
                ForEach(form.item.options.filter(\.isCatalogItem)) { option in
                    itemCard(option)
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
            // The category lens slides; the catalog underneath simply swaps.
            .transaction(value: form.category.value) { $0.animation = nil }
        }
        .sensoryFeedback(.selection, trigger: form.item.value)
        .safeAreaBar(edge: .top, spacing: 0) { itemSelectors }
        .scrollEdgeEffectStyle(.soft, for: .vertical)
    }

    private var itemSelectors: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                GlassEffectContainer(spacing: 8) {
                    HStack(spacing: 8) {
                        ForEach(form.category.options) { option in
                            choice(option.label, id: "kind-\(option.id)", lens: "family",
                                   selected: form.category.value == option.value) {
                                send(.category(option.value ?? ""))
                            }
                        }
                    }
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
            }
            .scrollClipDisabled()
            ScrollView(.horizontal, showsIndicators: false) {
                GlassEffectContainer(spacing: 7) {
                    HStack(spacing: 7) {
                        // The wildcard and Arcane Resin lead the item row; the
                        // catalog's own items are the grid below.
                        ForEach(form.item.options.filter { !$0.isCatalogItem }) { option in
                            if option.isArcaneResin {
                                choice(option.label, id: "resin", lens: "resin", selected: false) { pickResin() }
                            } else {
                                choice(option.label, id: "any", lens: "any",
                                       selected: !form.resinPicked && form.item.value == nil) {
                                    send(.item(nil))
                                }
                            }
                        }
                        if form.weaponType.visible {
                            ForEach(form.weaponType.options) { option in
                                choice(option.label, id: "subkind-\(option.id)", lens: "weapon-kind",
                                       selected: form.weaponType.value == option.value) {
                                    send(.weaponType(option.value ?? ""))
                                }
                            }
                        }
                    }
                }
                .padding(.horizontal, 18)
                .padding(.top, 4)
                .padding(.bottom, 12)
            }
            .scrollClipDisabled()
        }
    }

    private func itemCard(_ option: SheetOption) -> some View {
        let selected = !form.resinPicked && form.item.value == option.value
        return Button {
            withAnimation(AppTheme.glassSpring(reduceMotion)) { send(.item(option.value)) }
        } label: {
            VStack(spacing: 9) {
                ItemSpriteView(item: option.value.flatMap { ItemCatalog.findById($0) }, pointSize: 43)
                    .frame(height: 48)
                    .scaleEffect(selected && !reduceMotion ? 1.14 : 1)
                    .offset(y: selected && !reduceMotion ? -2 : 0)
                Text(option.label)
                    .font(.caption.weight(.medium))
                    .lineLimit(3)
                    .multilineTextAlignment(.center)
                    .frame(minHeight: 32)
            }
            .frame(maxWidth: .infinity, minHeight: 113)
            .padding(9)
            // The chosen item lifts out of the catalog into tinted glass.
            .background(selected ? .clear : Color(uiColor: .secondarySystemGroupedBackground),
                        in: .rect(cornerRadius: 25))
            .glassEffect(selected ? .regular.tint(AppTheme.accent.opacity(0.2)).interactive() : .identity,
                         in: .rect(cornerRadius: 25))
            .overlay {
                RoundedRectangle(cornerRadius: 25)
                    .strokeBorder(selected ? AppTheme.accent.opacity(0.55) : Color.white.opacity(0.035), lineWidth: 1)
            }
            .overlay(alignment: .topTrailing) {
                if selected {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(AppTheme.accent)
                        .padding(8)
                        .transition(.scale.combined(with: .opacity))
                }
            }
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private var detailsPage: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                if form.transmutations.visible || form.selectTrinket.visible { trinketControls }
                if form.tier.visible { tierControls }
                if form.upgrade.visible { upgradeControls }
                if form.effect.visible { effectControls }
                if form.uncursed.visible || form.source.visible || form.floorLimit.visible { placementControls }
                if form.excludeResin.visible {
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle(form.excludeResin.label, isOn: flag(form.excludeResin.value) { .excludeResin($0) })
                        explanation(form.excludeResin.caption)
                    }
                }
                // A cluster member's stack is its own, so its sheet shows it
                // like a lone chip's.
                if form.stack.visible { stackControls }
            }
            .padding(20)
        }
    }

    /// Transmutations, and choosing a trinket at +3.
    private var trinketControls: some View {
        VStack(alignment: .leading, spacing: 12) {
            if form.transmutations.visible {
                Toggle(form.transmutations.label,
                       isOn: flag(form.transmutations.enabled) { .transmutationsEnabled($0) })
                if form.transmutations.enabled {
                    // The value in words is the whole reading (`At most 3`).
                    Stepper(value: number(form.transmutations.value) { .transmutations($0) },
                            in: form.transmutations.range) {
                        Text(form.transmutations.valueLabel).foregroundStyle(.tint)
                    }
                }
                if form.transmutations.captionVisible { explanation(form.transmutations.caption) }
            }
            if form.selectTrinket.visible {
                Toggle(form.selectTrinket.label, isOn: flag(form.selectTrinket.value) { .selectTrinket($0) })
                explanation(form.selectTrinket.caption)
            }
        }
    }

    private var tierControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading("Tier")
            RequirementSegmentedControl(title: "Tier", options: form.tier.modes.map { ($0.value ?? "", $0.label) },
                                        selection: pick(form.tier.mode) { .tierMode($0) })
            if form.tier.valueVisible { valueSlider(form.tier) { .tier($0) } }
        }
    }

    private var upgradeControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading("Upgrade")
            RequirementSegmentedControl(title: "Upgrade", options: form.upgrade.modes.map { ($0.value ?? "", $0.label) },
                                        selection: pick(form.upgrade.mode) { .upgradeMode($0) })
            if form.upgrade.valueVisible { valueSlider(form.upgrade, isUpgrade: true) { .upgrade($0) } }
        }
    }

    /// A mode's value in words, and a slider over the bounds the core gives.
    private func valueSlider(_ control: SheetModeRange, isUpgrade: Bool = false,
                             _ change: @escaping @Sendable (Int) -> SheetChange) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            valueRow(control.modeLabel, control.valueLabel, isUpgrade: isUpgrade)
            if control.isAdjustable {
                RequirementGraduatedSlider(title: control.modeLabel, value: slider(control.value, change),
                                           bounds: Double(control.min)...Double(control.max))
            }
        }
    }

    private var effectControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading(form.effect.label)
            RequirementSegmentedControl(title: form.effect.label,
                                        options: form.effect.modes.map { ($0.value ?? "", $0.label) },
                                        selection: pick(form.effect.mode) { .effectMode($0) })
            if form.effect.choicesVisible {
                ForEach(form.effect.groups) { group in
                    effectGrid(heading: group.label, choices: form.effect.choices(in: group.value))
                }
                explanation(form.effect.caption)
            }
        }
    }

    private func effectGrid(heading: String, choices: [SheetEffectChoice]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 12) {
                Rectangle().fill(Color(uiColor: .separator)).frame(height: 0.5)
                Text(heading)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                    .fixedSize()
                Rectangle().fill(Color(uiColor: .separator)).frame(height: 0.5)
            }
            .padding(.vertical, 6)
            GlassEffectContainer(spacing: 8) {
                RequirementsFlowLayout(spacing: 8) {
                    ForEach(choices) { effect in
                        effectChip(effect, color: enchantmentGlows[effect.value].map(AppTheme.glowDisplayColor) ?? AppTheme.curse)
                    }
                }
            }
        }
        .sensoryFeedback(.selection, trigger: choices)
    }

    /// Each enchantment is glass tinted with the colour the item will glow in
    /// the game, so the chosen set reads like the pulsing sprite it describes.
    /// Its marker keeps one size, so choosing never reflows the chips.
    private func effectChip(_ effect: SheetEffectChoice, color: Color) -> some View {
        let selected = effect.selected
        return Button {
            withAnimation(AppTheme.glassSpring(reduceMotion)) { send(.toggleEffect(effect.value)) }
        } label: {
            HStack(spacing: 7) {
                ZStack {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 15, weight: .semibold))
                        .symbolRenderingMode(.palette)
                        .foregroundStyle(Color.black.opacity(0.75), color)
                        .scaleEffect(selected ? 1 : 0.4)
                        .opacity(selected ? 1 : 0)
                    Circle().fill(color).frame(width: 8, height: 8)
                        .opacity(selected ? 0 : 0.9)
                }
                .frame(width: 16, height: 16)
                Text(effect.label).font(.subheadline.weight(.medium))
                    .foregroundStyle(selected ? Color.primary : Color.primary.opacity(0.8))
            }
            .padding(.leading, 11).padding(.trailing, 14)
            .frame(minHeight: 40)
            .contentShape(.capsule)
            .glassEffect(.regular.tint(selected ? color.opacity(0.34) : nil).interactive(), in: .capsule)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(effect.label)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private var placementControls: some View {
        VStack(alignment: .leading, spacing: 16) {
            if form.uncursed.visible {
                Toggle(form.uncursed.label, isOn: flag(form.uncursed.value) { .uncursed($0) })
                explanation(form.uncursed.caption)
            }
            if form.source.visible {
                RequirementSourceSelector(options: form.source.options,
                                          selection: pickOptional(form.source.value) { .source($0) })
            }
            if form.floorLimit.visible {
                RequirementsFloorControl(control: form.floorLimit,
                                         enabled: flag(form.floorLimit.enabled) { .floorLimitEnabled($0) },
                                         floor: number(form.floorLimit.value) { .floorLimit($0) })
            }
        }
    }

    private var stackControls: some View {
        VStack(alignment: .leading, spacing: 14) {
            Stepper(value: number(form.stack.count) { .count($0) }, in: form.stack.range) {
                HStack { heading(form.stack.label); Spacer(); Text(form.stack.valueLabel).foregroundStyle(.tint) }
            }
            if form.stack.copyDepth.visible {
                RequirementsFloorControl(control: form.stack.copyDepth,
                                         enabled: flag(form.stack.copyDepth.enabled) { .copyDepthEnabled($0) },
                                         floor: number(form.stack.copyDepth.value) { .copyDepth($0) })
            }
            if form.stack.countLevels.visible {
                Toggle(form.stack.countLevels.label, isOn: flag(form.stack.countLevels.enabled) { .countLevels($0) })
                if form.stack.countLevels.captionVisible { explanation(form.stack.countLevels.caption) }
                if form.stack.countLevels.enabled {
                    // The value in words is the whole reading (`≥ 5 across up to 2`).
                    HStack {
                        Spacer()
                        Text(form.stack.countLevels.valueLabel).font(.subheadline).foregroundStyle(.tint)
                    }
                    if form.stack.countLevels.isAdjustable {
                        RequirementGraduatedSlider(title: form.stack.countLevels.label,
                                                   value: slider(form.stack.countLevels.value) { .total($0) },
                                                   bounds: Double(form.stack.countLevels.min)...Double(form.stack.countLevels.max))
                            .accessibilityValue(form.stack.countLevels.valueLabel)
                    }
                }
            }
        }
    }

    private var footer: some View {
        GlassEffectContainer(spacing: 18) {
            VStack(spacing: 12) {
                if details {
                    requirementPreview
                        .glassEffectID("preview", in: editorGlass)
                        .transition(.move(edge: .bottom).combined(with: .opacity))
                    HStack(spacing: 10) {
                        Button { showItemPage() } label: {
                            Label("Back", systemImage: "chevron.left")
                                .font(.subheadline.weight(.medium))
                                .frame(minWidth: 66, minHeight: 48)
                                .padding(.horizontal, 12)
                                .glassEffect(.regular.interactive(), in: .capsule)
                        }
                        .buttonStyle(.plain)
                        .glassEffectID("back", in: editorGlass)
                        if let onRemove {
                            Button(role: .destructive) { onRemove(); dismiss() } label: {
                                Image(systemName: "trash")
                                    .font(.system(size: 19, weight: .medium))
                                    .foregroundStyle(.red)
                                    .frame(width: 48, height: 48)
                                    .glassEffect(.regular.tint(.red.opacity(0.07)).interactive(), in: .circle)
                            }
                            .buttonStyle(.plain)
                            .tint(.red)
                            .accessibilityLabel(form.inCluster ? "Remove alternative" : "Remove requirement")
                            .glassEffectID("remove", in: editorGlass)
                        }
                        Spacer(minLength: 12)
                        primaryAction(form.mode == .new ? "Add" : "Save", symbol: "checkmark", action: save)
                            .disabled(!form.canSave)
                            .opacity(form.canSave ? 1 : 0.45)
                    }
                } else {
                    HStack {
                        Spacer()
                        primaryAction("Next", symbol: "arrow.right") {
                            withAnimation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.82)) { details = true }
                        }
                    }
                }
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 12)
        .padding(.bottom, 10)
    }

    /// The chip the save would put on the board, or why it cannot be saved.
    private var requirementPreview: some View {
        HStack(spacing: 12) {
            if let preview = form.preview {
                RequirementsChipSprite(face: preview.face, size: 36)
                VStack(alignment: .leading, spacing: 4) {
                    Text(preview.title).font(.subheadline.weight(.semibold))
                    if !preview.details.isEmpty {
                        Text(preview.details.joined(separator: " · "))
                            .font(.caption).foregroundStyle(.secondary)
                            .contentTransition(.numericText())
                    }
                }
                .animation(reduceMotion ? nil : .snappy, value: preview.details)
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                Text(form.errors.joined(separator: "\n"))
                    .font(.caption).foregroundStyle(.red).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.horizontal, 17)
        .padding(.vertical, 14)
        .glassEffect(.regular, in: .rect(cornerRadius: 26))
    }

    private func primaryAction(_ title: String, symbol: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 9) {
                Text(title)
                Image(systemName: symbol).font(.subheadline.weight(.semibold))
            }
            .font(.headline)
            .foregroundStyle(.primary)
            .frame(minHeight: 52)
            .padding(.horizontal, 23)
            .glassEffect(.regular.tint(AppTheme.accent.opacity(0.3)).interactive(), in: .capsule)
        }
        .buttonStyle(.plain)
        .glassEffectID("primary", in: editorGlass)
    }

    private func showItemPage() {
        withAnimation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.82)) { details = false }
    }

    /// Each row's selection is one lens of tinted glass that slides to the
    /// chosen capsule instead of re-tinting it in place.
    private func choice(_ text: String, id: String, lens: String, selected: Bool,
                        action: @escaping () -> Void) -> some View {
        Button {
            withAnimation(AppTheme.glassSpring(reduceMotion)) { action() }
        } label: {
            Text(text).font(.subheadline.weight(.medium))
                .fixedSize().padding(.horizontal, 16).frame(minHeight: 44)
                .foregroundStyle(selected ? Color.primary : Color.primary.opacity(0.8))
                .contentShape(.capsule)
                .glassChoice(id, selected: selected, lens: lens, in: selectorGlass)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private func heading(_ value: String) -> some View { Text(value).font(.subheadline.weight(.semibold)) }
    /// A help text of the form's, under what it explains; nothing for none.
    @ViewBuilder private func explanation(_ value: String?) -> some View {
        if let value {
            Text(value).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
    }
    private func valueRow(_ title: String, _ value: String, isUpgrade: Bool = false) -> some View {
        HStack {
            Text(title)
            Spacer()
            Text(value)
                .foregroundStyle(isUpgrade ? AppTheme.upgrade : AppTheme.accent)
                .fontWeight(isUpgrade ? .bold : .regular)
        }
        .font(.subheadline)
    }

    /// Arcane Resin is the query's own condition, edited on its own sheet:
    /// the picked resin goes there, with the chip it may replace.
    private func pickResin() {
        guard let picked = sheet.changing(.item(RequirementSheet.arcaneResin)), picked.form.resinPicked else { return }
        sheet = picked
        onPickResin(picked)
    }

    private func save() {
        guard saveDraft() else { return }
        dismiss()
    }

    private func saveDraft() -> Bool {
        guard let refused = onSave(sheet) else { return true }
        sheet = refused
        return false
    }

    // MARK: Changes

    /// Sends one change; the core's answer is the sheet shown next. A change
    /// the core cannot apply leaves the sheet as it was.
    private func send(_ change: SheetChange) {
        if let next = sheet.changing(change) { sheet = next }
    }

    // Each control reads the form and sends a change when it moves — none
    // when it lands on the value shown, since sliders repeat theirs.

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

    private func slider(_ value: Int, _ change: @escaping @Sendable (Int) -> SheetChange) -> Binding<Double> {
        Binding(get: { Double(value) }, set: { next in
            let rounded = Int(next.rounded())
            if rounded != value { send(change(rounded)) }
        })
    }

    private func pick(_ value: String, _ change: @escaping @Sendable (String) -> SheetChange) -> Binding<String> {
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

/// A floor limit as the shared core offers it: its switch and, while it is
/// on, a slider over the floors it stops at, which skip the empty boss floors.
struct RequirementsFloorControl: View {
    let control: SheetFloorToggle
    @Binding var enabled: Bool
    @Binding var floor: Int

    private var position: Binding<Double> {
        Binding(get: { Double(control.index) }, set: { value in
            if let next = control.floor(at: Int(value.rounded())) { floor = next }
        })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Toggle(control.label, isOn: $enabled)
            if control.enabled {
                HStack {
                    Spacer()
                    Text(control.valueLabel).font(.subheadline).foregroundStyle(.tint)
                }
                RequirementGraduatedSlider(title: control.label, value: position,
                                           bounds: 0...Double(max(1, control.options.count - 1)))
                    .accessibilityValue(control.valueLabel)
            }
        }
    }
}
