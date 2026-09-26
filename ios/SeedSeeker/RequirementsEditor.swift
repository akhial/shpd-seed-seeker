import SwiftUI
import SeedSeekerKit

struct RequirementsEditor: View {
    @Environment(\.dismiss) private var dismiss
    let editing: ItemRequirement?
    let otherRequirements: [ItemRequirement]
    let blanket: Bool
    let onAddResin: (() -> Void)?
    let onEditGroupQuantity: (() -> Void)?
    let onSave: (ItemRequirement, Int, Int?, Int?) -> Void
    let onRemove: (() -> Void)?

    @State private var details: Bool
    @State private var kind: ItemKind
    @State private var selectedItem: CatalogItem?
    @State private var tierMatch: TierMatch
    @State private var tier: Int
    @State private var upgradeMatch: UpgradeMatch
    @State private var upgrade: Int
    @State private var effectMode: Int
    @State private var selectedEffects: Set<String>
    @State private var source: ScoutItemSource?
    @State private var maximumDepth: Int?
    @State private var requireUncursed: Bool
    @State private var selectTrinket: Bool
    @State private var trinketTransmutations: Int
    @State private var artifactTransmutations: Int
    @State private var excludeResin: Bool
    @State private var stackCount: Int
    @State private var stackTotal: Int?
    @State private var copyDepth: Int?
    @Namespace private var editorGlass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(editing: ItemRequirement?, otherRequirements: [ItemRequirement], blanket: Bool,
         editingCount: Int = 1, editingTotal: Int? = nil, editingCopyDepth: Int? = nil,
         onAddResin: (() -> Void)? = nil,
         onEditGroupQuantity: (() -> Void)? = nil,
         onSave: @escaping (ItemRequirement, Int, Int?, Int?) -> Void,
         onRemove: (() -> Void)? = nil) {
        self.editing = editing
        self.otherRequirements = otherRequirements
        self.blanket = blanket
        self.onAddResin = onAddResin
        self.onEditGroupQuantity = onEditGroupQuantity
        self.onSave = onSave
        self.onRemove = onRemove
        _details = State(initialValue: editing != nil)
        let kind = editing?.kind ?? .weapon
        _kind = State(initialValue: kind)
        _selectedItem = State(initialValue: editing == nil ? (blanket && kind != .trinket && kind != .artifact ? nil : Self.items(for: kind).first) : editing?.item)
        _tierMatch = State(initialValue: editing?.tierMatch ?? .any)
        _tier = State(initialValue: max(2, editing?.tier ?? 2))
        _upgradeMatch = State(initialValue: editing?.upgradeMatch ?? .exactly)
        _upgrade = State(initialValue: editing?.upgrade ?? 1)
        _effectMode = State(initialValue: editing?.effect == .anyEnchantment ? 1 : editing?.effect.names.isEmpty == false ? 2 : 0)
        _selectedEffects = State(initialValue: Set(editing?.effect.names ?? []))
        _source = State(initialValue: editing?.source)
        _maximumDepth = State(initialValue: editing?.maximumDepth)
        _requireUncursed = State(initialValue: editing?.requireUncursed ?? false)
        _selectTrinket = State(initialValue: editing?.selectTrinket ?? false)
        _trinketTransmutations = State(initialValue: editing?.trinketTransmutations ?? 0)
        _artifactTransmutations = State(initialValue: editing?.artifactTransmutations ?? 0)
        _excludeResin = State(initialValue: editing?.excludeResin ?? false)
        _stackCount = State(initialValue: min(SearchLimits.stackMax, max(1, editingCount)))
        _stackTotal = State(initialValue: editingTotal)
        _copyDepth = State(initialValue: editingCopyDepth)
    }

    private var namedOnly: Bool { kind == .trinket || kind == .artifact }
    private var inAlternative: Bool { editing?.alternativeGroup != nil }
    private var ceiling: Int {
        SearchLimits.maximumUpgrade(kind: kind, item: selectedItem,
                                    tier: tierMatch == .any ? 0 : tier, tierMatch: tierMatch)
    }
    private var levelCapacity: Int {
        min(((upgradeMatch == .exactly ? upgrade : ceiling) + 1) * stackCount,
            SearchLimits.ringStackCapacity(stackCount))
    }
    private var title: String {
        if blanket { return editing == nil ? "Add blanket requirement" : "Edit blanket requirement" }
        if editing == nil { return "Add requirement" }
        return inAlternative ? "Edit alternative" : "Edit requirement"
    }
    private var duplicateTrinket: Bool {
        !blanket && kind == .trinket && selectedItem != nil &&
        otherRequirements.contains { !$0.blanket && $0.item?.id == selectedItem?.id }
    }
    private var effect: EffectFilter {
        if effectMode == 1 { return .anyEnchantment }
        guard effectMode == 2 else { return .any }
        let ordered = ItemCatalog.modifiersFor(kind).filter { selectedEffects.contains($0) }
        return ordered.isEmpty ? .any : .oneOf(ordered)
    }
    private var draft: ItemRequirement? {
        guard !duplicateTrinket else { return nil }
        return try? ItemRequirement(
            key: editing?.key ?? 0, item: selectedItem, upgrade: namedOnly ? 0 : upgrade,
            effect: effect, kind: kind, tier: tierMatch == .any ? 0 : tier,
            tierMatch: tierMatch, upgradeMatch: namedOnly ? .any : upgradeMatch,
            source: kind == .trinket ? nil : source,
            identityGroup: blanket || namedOnly ? nil : editing?.identityGroup,
            maximumDepth: kind == .trinket ? nil : maximumDepth,
            requireUncursed: kind != .trinket && requireUncursed,
            alternativeGroup: editing?.alternativeGroup,
            selectTrinket: !blanket && kind == .trinket && trinketTransmutations == 0 && selectTrinket,
            trinketTransmutations: kind == .trinket ? trinketTransmutations : 0,
            artifactTransmutations: kind == .artifact ? artifactTransmutations : 0,
            blanket: blanket, excludeResin: !blanket && kind == .wand && excludeResin
        )
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
            .onAppear { normalizeBounds() }
            .onChange(of: upgradeMatch) { _, _ in normalizeBounds() }
            .onChange(of: tierMatch) { _, match in
                if match == .atLeast || match == .atMost { tier = min(4, max(3, tier)) }
                normalizeBounds()
            }
            .onChange(of: tier) { _, _ in normalizeBounds() }
            .onChange(of: upgrade) { _, _ in clampTotal() }
            .onChange(of: stackCount) { _, _ in clampTotal() }
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
    }

    private var itemPage: some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 100), spacing: 10)], spacing: 10) {
                ForEach(Self.items(for: kind)) { item in
                    itemCard(item)
                }
            }
            .padding(.horizontal, 18)
            .padding(.vertical, 12)
        }
        .safeAreaBar(edge: .top, spacing: 0) { itemSelectors }
        .scrollEdgeEffectStyle(.soft, for: .vertical)
    }

    private var itemSelectors: some View {
        VStack(spacing: 0) {
            ScrollView(.horizontal, showsIndicators: false) {
                GlassEffectContainer(spacing: 8) {
                    HStack(spacing: 8) {
                        ForEach([ItemKind.weapon, .armor, .wand, .ring, .trinket, .artifact], id: \.rawValue) { entry in
                            choice(entry.label, selected: kind.family == entry) { changeKind(entry) }
                        }
                    }
                }
                .padding(.horizontal, 18)
                .padding(.vertical, 10)
            }
            .scrollClipDisabled()
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 7) {
                    if !namedOnly {
                        choice("Any \(kind.label.lowercased())", selected: selectedItem == nil) {
                            selectedItem = nil
                            normalizeBounds()
                        }
                    }
                    if kind == .wand, let onAddResin {
                        choice("Arcane Resin", selected: false, action: onAddResin)
                    }
                    if kind.family == .weapon {
                        ForEach([ItemKind.weapon, .meleeWeapon, .thrownWeapon], id: \.rawValue) { entry in
                            choice(entry == .weapon ? "All" : entry == .meleeWeapon ? "Melee" : "Thrown", selected: kind == entry) {
                                if kind != entry {
                                    kind = entry
                                    if selectedItem.map(entry.accepts) != true { selectedItem = Self.items(for: entry).first }
                                    tierMatch = .any
                                    normalizeBounds()
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

    private func itemCard(_ item: CatalogItem) -> some View {
        let selected = selectedItem?.id == item.id
        return Button {
            withAnimation(reduceMotion ? nil : .snappy(duration: 0.25)) {
                selectedItem = item
                tierMatch = .any
                normalizeBounds()
            }
        } label: {
            VStack(spacing: 9) {
                ItemSpriteView(item: item, pointSize: 43).frame(height: 48)
                Text(item.name)
                    .font(.caption.weight(.medium))
                    .lineLimit(3)
                    .multilineTextAlignment(.center)
                    .frame(minHeight: 32)
            }
            .frame(maxWidth: .infinity, minHeight: 113)
            .padding(9)
            .background(selected ? Color.accentColor.opacity(0.1) : Color(uiColor: .secondarySystemGroupedBackground),
                        in: .rect(cornerRadius: 25))
            .overlay {
                RoundedRectangle(cornerRadius: 25)
                    .strokeBorder(selected ? Color.accentColor.opacity(0.7) : Color.white.opacity(0.035), lineWidth: selected ? 1.5 : 1)
            }
            .overlay(alignment: .topTrailing) {
                if selected {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(Color.accentColor)
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
                if kind == .trinket { trinketControls }
                if kind == .artifact { artifactControls }
                if selectedItem == nil && (kind.family == .weapon || kind.family == .armor) { tierControls }
                if !namedOnly { upgradeControls }
                if let label = kind.modifierLabel { effectControls(label: label) }
                if kind != .trinket { placementControls }
                if !blanket && kind == .wand {
                    VStack(alignment: .leading, spacing: 8) {
                        Toggle("Exclude from Auto resin", isOn: $excludeResin)
                        explanation("Keep this wand without budgeting resin to upgrade it. Useful for imbuing: resin upgrades do not transfer to the staff. Extra copies are reserved for reforging and never need Auto resin.")
                    }
                }
                if !blanket && !inAlternative && !namedOnly { stackControls }
                if let onEditGroupQuantity, !namedOnly { groupQuantityButton(action: onEditGroupQuantity) }
            }
            .padding(20)
        }
    }

    private var trinketControls: some View {
        VStack(alignment: .leading, spacing: 12) {
            Toggle("Allow transmutations", isOn: Binding(get: { trinketTransmutations > 0 }, set: {
                trinketTransmutations = $0 ? 1 : 0
                if $0 { selectTrinket = false }
            }))
            if trinketTransmutations > 0 {
                Stepper(value: $trinketTransmutations, in: 1...13) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Maximum transmutations")
                        Text("At most \(trinketTransmutations)").foregroundStyle(.tint)
                    }
                }
                explanation("Includes the initial offers. AutoTrinket can use a helpful starting trinket. Scroll availability and effects after transmuting are not simulated.")
            } else if !blanket {
                Toggle("Choose matching trinket at +3", isOn: $selectTrinket)
                explanation("Applies after the first brewing opportunity. If several alternatives are offered, no trinket is chosen.")
            }
        }
    }

    private var artifactControls: some View {
        VStack(alignment: .leading, spacing: 12) {
            Toggle("Allow transmutations", isOn: Binding(get: { artifactTransmutations > 0 }, set: {
                artifactTransmutations = $0 ? 1 : 0
            }))
            if artifactTransmutations > 0 {
                Stepper(value: $artifactTransmutations, in: 1...10) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Maximum transmutations")
                        Text("At most \(artifactTransmutations)").foregroundStyle(.tint)
                    }
                }
                explanation("Includes natural finds or transforms an obtainable artifact using the remaining deck at the floor limit. Source and curse filters apply to the starting artifact. Scroll availability and later generation changes are not simulated.")
            }
        }
    }

    private var tierControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading("Tier")
            RequirementSegmentedControl(title: "Tier", options: TierMatch.allCases.map { ($0, $0.label) }, selection: $tierMatch)
            if tierMatch == .exactly {
                valueRow("Exact tier", "Tier \(tier)")
                RequirementGraduatedSlider(title: "Exact tier", value: Binding(get: { Double(tier) }, set: { tier = Int($0.rounded()) }), bounds: 2...5)
            } else if tierMatch != .any {
                Picker(tierMatch == .atLeast ? "Minimum tier" : "Maximum tier", selection: $tier) {
                    ForEach(Array(SearchLimits.boundedTiers), id: \.self) { value in Text("Tier \(value)").tag(value) }
                }.pickerStyle(.menu)
            }
        }
    }

    private var upgradeControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading("Upgrade")
            RequirementSegmentedControl(title: "Upgrade", options: UpgradeMatch.allCases.map { ($0, $0.label) }, selection: $upgradeMatch)
            if upgradeMatch == .exactly {
                valueRow("Level", "+\(upgrade)", isUpgrade: true)
                RequirementGraduatedSlider(title: "Level", value: Binding(get: { Double(upgrade) }, set: { upgrade = Int($0.rounded()) }),
                                           bounds: 1...Double(max(2, ceiling)))
            } else if upgradeMatch == .atLeast {
                valueRow("At least", "+\(upgrade) or higher", isUpgrade: true)
                RequirementGraduatedSlider(title: "Minimum upgrade", value: Binding(get: { Double(upgrade) }, set: { upgrade = Int($0.rounded()) }),
                                           bounds: 1...Double(max(2, ceiling - 1)))
            }
        }
    }

    private func effectControls(label: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            heading(label)
            RequirementSegmentedControl(title: label,
                                        options: [(0, "Any"), (1, "Any \(label.lowercased())"), (2, "Specific…")],
                                        selection: $effectMode)
            if effectMode == 2 {
                effectGrid(heading: kind.family == .weapon ? "Enchantments" : "Glyphs", names: kind.enchantmentNames)
                if !requireUncursed { effectGrid(heading: "Curses", names: ItemCatalog.cursesFor(kind)) }
                if selectedEffects.isEmpty {
                    explanation("Nothing picked yet — any \(label.lowercased()) is accepted until you do.")
                }
            }
        }
    }

    private func effectGrid(heading: String, names: [String]) -> some View {
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
            LazyVGrid(columns: [GridItem(.flexible(), alignment: .leading), GridItem(.flexible(), alignment: .leading)], alignment: .leading, spacing: 7) {
                ForEach(names, id: \.self) { name in
                    Button {
                        if selectedEffects.contains(name) { selectedEffects.remove(name) } else { selectedEffects.insert(name) }
                    } label: {
                        HStack(spacing: 7) {
                            Image(systemName: selectedEffects.contains(name) ? "checkmark.circle.fill" : "circle")
                                .foregroundStyle(selectedEffects.contains(name) ? Color.accentColor : .secondary)
                            Text(name).font(.subheadline).foregroundStyle(.primary)
                        }.frame(maxWidth: .infinity, alignment: .leading).padding(.vertical, 5)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selectedEffects.contains(name) ? [.isSelected] : [])
                }
            }
        }
    }

    private var placementControls: some View {
        VStack(alignment: .leading, spacing: 16) {
            Toggle("Require uncursed", isOn: Binding(get: { requireUncursed }, set: { value in
                requireUncursed = value
                if value { selectedEffects.subtract(ItemCatalog.cursesFor(kind)) }
            }))
            RequirementSourceSelector(source: $source)
            RequirementsFloorPicker(title: "Floor limit", depth: $maximumDepth)
        }
    }

    private var stackControls: some View {
        VStack(alignment: .leading, spacing: 14) {
            Stepper(value: $stackCount, in: 1...SearchLimits.stackMax) {
                HStack { heading("How many"); Spacer(); Text("×\(stackCount)").foregroundStyle(.tint) }
            }
            if stackCount > 1 && stackTotal == nil {
                Toggle("Limit the extra copies to a floor", isOn: Binding(get: { copyDepth != nil }, set: { copyDepth = $0 ? 4 : nil }))
                explanation("A floor limit is where an item lies, not what it is, so the copies keep their own.")
                if copyDepth != nil { RequirementsFloorPicker(title: "Copies within first", depth: $copyDepth, allowsNone: false) }
            }
            if stackCount > 1 && selectedItem != nil && kind == .ring {
                Toggle("Combined level", isOn: Binding(get: { stackTotal != nil }, set: { stackTotal = $0 ? max(1, levelCapacity) : nil }))
                explanation("Each item counts its upgrade plus one, and spare items may go unused.")
                if let total = stackTotal {
                    valueRow("Levels together", "≥ \(total) of \(levelCapacity)")
                    RequirementGraduatedSlider(title: "Combined level at least \(total)",
                                               value: Binding(get: { Double(stackTotal ?? 1) }, set: { stackTotal = Int($0.rounded()) }),
                                               bounds: 1...Double(max(2, levelCapacity)))
                }
            }
        }
    }

    private func groupQuantityButton(action: @escaping () -> Void) -> some View {
        Button {
            if saveDraft() { action() }
        } label: {
            HStack(spacing: 12) {
                Text("How many").foregroundStyle(.primary)
                Spacer(minLength: 8)
                Text("×\(stackCount)").foregroundStyle(.secondary)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            .font(.subheadline)
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 18))
        }
        .buttonStyle(.plain)
        .disabled(draft == nil)
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
                            .accessibilityLabel(inAlternative ? "Remove alternative" : "Remove requirement")
                            .glassEffectID("remove", in: editorGlass)
                        }
                        Spacer(minLength: 12)
                        primaryAction(editing == nil ? "Add" : "Save", symbol: "checkmark", action: save)
                            .disabled(draft == nil)
                            .opacity(draft == nil ? 0.45 : 1)
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

    private var requirementPreview: some View {
        HStack(spacing: 12) {
            if let draft {
                RequirementsSprite(requirement: draft, size: 36)
                VStack(alignment: .leading, spacing: 4) {
                    Text(draft.title).font(.subheadline.weight(.semibold))
                    Text(draft.description.replacingOccurrences(of: " • ", with: " · "))
                        .font(.caption).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            } else {
                Text(duplicateTrinket ? "This trinket is already required. Each trinket appears only once in the deck." : "This requirement cannot be saved.")
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
            .glassEffect(.regular.tint(Color.accentColor.opacity(0.3)).interactive(), in: .capsule)
        }
        .buttonStyle(.plain)
        .glassEffectID("primary", in: editorGlass)
    }

    private func showItemPage() {
        withAnimation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.82)) { details = false }
    }

    private func choice(_ text: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(text).font(.subheadline.weight(selected ? .semibold : .regular))
                .fixedSize().padding(.horizontal, 16).frame(minHeight: 44)
                .foregroundStyle(.primary)
                .glassEffect(.regular.tint(selected ? Color.accentColor.opacity(0.2) : .clear).interactive(), in: .capsule)
        }.buttonStyle(.plain).accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private func heading(_ value: String) -> some View { Text(value).font(.subheadline.weight(.semibold)) }
    private func explanation(_ value: String) -> some View { Text(value).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
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

    private func changeKind(_ entry: ItemKind) {
        guard kind.family != entry else { return }
        kind = entry
        selectedItem = Self.items(for: entry).first
        tierMatch = .any
        tier = 2
        effectMode = 0
        selectedEffects = []
        selectTrinket = false
        trinketTransmutations = 0
        artifactTransmutations = 0
        if namedOnly {
            stackCount = 1
            stackTotal = nil
            copyDepth = nil
            upgradeMatch = .any
            upgrade = 0
            source = nil
            requireUncursed = false
        }
        if entry != .ring { stackTotal = nil }
        normalizeBounds()
    }

    private func normalizeBounds() {
        if namedOnly || upgradeMatch == .any { upgrade = 0 }
        else { upgrade = min(max(1, upgrade), upgradeMatch == .exactly ? ceiling : max(1, ceiling - 1)) }
        clampTotal()
    }

    private func clampTotal() {
        if let total = stackTotal { stackTotal = min(total, max(1, levelCapacity)) }
    }

    private func save() {
        guard saveDraft() else { return }
        dismiss()
    }

    private func saveDraft() -> Bool {
        guard let draft else { return false }
        let total = !blanket && !inAlternative && selectedItem != nil && kind == .ring && stackCount > 1 ? stackTotal : nil
        let copies = !blanket && !inAlternative && stackCount > 1 && total == nil ? copyDepth : nil
        onSave(draft, !blanket && !namedOnly ? stackCount : 1, total, copies)
        return true
    }

    private static func items(for kind: ItemKind) -> [CatalogItem] {
        ItemCatalog.forKind(kind).filter { $0.tier != 1 && !$0.isTippedDart }
    }
}

struct RequirementsFloorPicker: View {
    let title: String
    @Binding var depth: Int?
    var allowsNone = true

    private var selection: Binding<Double> {
        Binding(get: {
            guard let depth else { return 0 }
            return Double(FloorLimits.index(of: depth) + (allowsNone ? 1 : 0))
        }, set: { value in
            let index = Int(value.rounded()) - (allowsNone ? 1 : 0)
            depth = index < 0 ? nil : FloorLimits.options[min(index, FloorLimits.options.count - 1)]
        })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                Text(title).font(.subheadline.weight(.semibold))
                Spacer()
                Text(depth.map { allowsNone ? "≤ floor \($0)" : "\($0) floor\($0 == 1 ? "" : "s")" } ?? "Search limit")
                    .font(.subheadline).foregroundStyle(.tint)
            }
            RequirementGraduatedSlider(title: title, value: selection,
                                       bounds: 0...Double(FloorLimits.options.count - (allowsNone ? 0 : 1)))
                .accessibilityValue(depth.map { "Floor \($0)" } ?? "No limit")
        }
    }
}
