import SwiftUI
import SeedSeekerKit

struct RequirementsEditor: View {
    @Environment(\.dismiss) private var dismiss
    let editing: ItemRequirement?
    let otherRequirements: [ItemRequirement]
    let blanket: Bool
    let onAddResin: (() -> Void)?
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
    @State private var excludeResin: Bool
    @State private var stackCount: Int
    @State private var stackTotal: Int?
    @State private var copyDepth: Int?

    init(editing: ItemRequirement?, otherRequirements: [ItemRequirement], blanket: Bool,
         editingCount: Int = 1, editingTotal: Int? = nil, editingCopyDepth: Int? = nil,
         onAddResin: (() -> Void)? = nil,
         onSave: @escaping (ItemRequirement, Int, Int?, Int?) -> Void,
         onRemove: (() -> Void)? = nil) {
        self.editing = editing
        self.otherRequirements = otherRequirements
        self.blanket = blanket
        self.onAddResin = onAddResin
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
            blanket: blanket, excludeResin: !blanket && kind == .wand && excludeResin
        )
    }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                Text(kind == .trinket ? "Trinket" : details ? "2/2 · Details" : "1/2 · Item")
                    .font(.caption).foregroundStyle(.secondary).padding(.bottom, 8)
                if details { detailsPage } else { itemPage }
                footer
            }
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
        VStack(spacing: 10) {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach([ItemKind.weapon, .armor, .wand, .ring, .trinket, .artifact], id: \.rawValue) { entry in
                        choice(entry.label, selected: kind.family == entry) { changeKind(entry) }
                    }
                }.padding(.horizontal, 16)
            }
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
                }.padding(.horizontal, 16)
            }
            ScrollView {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 96), spacing: 8)], spacing: 8) {
                    ForEach(Self.items(for: kind)) { item in
                        Button {
                            selectedItem = item
                            tierMatch = .any
                            normalizeBounds()
                        } label: {
                            VStack(spacing: 6) {
                                ItemSpriteView(item: item, pointSize: 42).frame(height: 44)
                                Text(item.name).font(.caption).fontWeight(.medium)
                                    .lineLimit(2).multilineTextAlignment(.center).frame(minHeight: 28)
                                if let tier = item.tier {
                                    Text("Tier \(tier)").font(.caption2).foregroundStyle(.secondary)
                                }
                            }
                            .frame(maxWidth: .infinity, minHeight: 107)
                            .padding(7)
                            .background(selectedItem?.id == item.id ? Color.accentColor.opacity(0.18) : Color(uiColor: .secondarySystemGroupedBackground),
                                        in: RoundedRectangle(cornerRadius: 18))
                            .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(selectedItem?.id == item.id ? Color.accentColor : .clear, lineWidth: 1.5))
                        }
                        .buttonStyle(.plain)
                        .accessibilityAddTraits(selectedItem?.id == item.id ? [.isSelected] : [])
                    }
                }.padding(.horizontal, 16).padding(.vertical, 4)
            }
        }
    }

    private var detailsPage: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                if kind == .trinket { trinketControls }
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

    private var tierControls: some View {
        VStack(alignment: .leading, spacing: 10) {
            heading("Tier")
            Picker("Tier", selection: $tierMatch) {
                ForEach(TierMatch.allCases, id: \.rawValue) { match in Text(match.label).tag(match) }
            }.pickerStyle(.segmented)
            if tierMatch == .exactly {
                valueRow("Exact tier", "Tier \(tier)")
                Slider(value: Binding(get: { Double(tier) }, set: { tier = Int($0.rounded()) }), in: 2...5, step: 1)
                    .accessibilityLabel("Exact tier")
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
            Picker("Upgrade", selection: $upgradeMatch) {
                ForEach(UpgradeMatch.allCases, id: \.rawValue) { match in Text(match.label).tag(match) }
            }.pickerStyle(.segmented)
            if upgradeMatch == .exactly {
                valueRow("Level", "+\(upgrade)")
                Slider(value: Binding(get: { Double(upgrade) }, set: { upgrade = Int($0.rounded()) }),
                       in: 1...Double(max(2, ceiling)), step: 1).accessibilityLabel("Level")
            } else if upgradeMatch == .atLeast {
                valueRow("At least", "+\(upgrade) or higher")
                Slider(value: Binding(get: { Double(upgrade) }, set: { upgrade = Int($0.rounded()) }),
                       in: 1...Double(max(2, ceiling - 1)), step: 1).accessibilityLabel("Minimum upgrade")
            }
        }
    }

    private func effectControls(label: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            heading(label)
            Picker(label, selection: $effectMode) {
                Text("Any").tag(0)
                Text("Any \(label.lowercased())").tag(1)
                Text("Specific…").tag(2)
            }.pickerStyle(.segmented)
            if effectMode == 2 {
                effectGrid(heading: kind.family == .weapon ? "ENCHANTMENTS" : "GLYPHS", names: kind.enchantmentNames)
                if !requireUncursed { effectGrid(heading: "CURSES", names: ItemCatalog.cursesFor(kind)) }
                if selectedEffects.isEmpty {
                    explanation("Nothing picked yet — any \(label.lowercased()) is accepted until you do.")
                }
            }
        }
    }

    private func effectGrid(heading: String, names: [String]) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(heading).font(.caption2.weight(.semibold)).tracking(1).foregroundStyle(.secondary)
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
            Picker("Source", selection: $source) {
                Text("Any source").tag(Optional<ScoutItemSource>.none)
                ForEach(ScoutItemSource.allCases, id: \.rawValue) { option in Text(option.label).tag(Optional(option)) }
            }.pickerStyle(.menu)
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
                    Slider(value: Binding(get: { Double(stackTotal ?? 1) }, set: { stackTotal = Int($0.rounded()) }),
                           in: 1...Double(max(2, levelCapacity)), step: 1)
                        .accessibilityLabel("Combined level at least \(total)")
                }
            }
        }
    }

    private var footer: some View {
        VStack(spacing: 12) {
            if details {
                HStack(spacing: 12) {
                    if let draft {
                        RequirementsSprite(requirement: draft, size: 40)
                        VStack(alignment: .leading, spacing: 3) {
                            Text(draft.title).font(.subheadline.weight(.semibold))
                            Text(draft.description.replacingOccurrences(of: " • ", with: " · "))
                                .font(.caption).foregroundStyle(.secondary)
                        }.frame(maxWidth: .infinity, alignment: .leading)
                    } else {
                        Text(duplicateTrinket ? "This trinket is already required. Each trinket appears only once in the deck." : "This requirement cannot be saved.")
                            .font(.caption).foregroundStyle(.red).frame(maxWidth: .infinity, alignment: .leading)
                    }
                }
                .padding(12)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 18))
            }
            HStack(spacing: 12) {
                if details {
                    if let onRemove {
                        Button(role: .destructive) { onRemove(); dismiss() } label: { Image(systemName: "trash").frame(minHeight: 30) }
                            .buttonStyle(.glass)
                            .accessibilityLabel(inAlternative ? "Remove alternative" : "Remove requirement")
                    }
                    Button("Back") { details = false }.buttonStyle(.glass).controlSize(.large)
                    Button(editing == nil ? "Add" : "Save", action: save)
                        .buttonStyle(.glassProminent).controlSize(.large).frame(maxWidth: .infinity)
                        .disabled(draft == nil)
                } else {
                    Button { details = true } label: { Text("Next").frame(maxWidth: .infinity) }
                        .buttonStyle(.glassProminent).controlSize(.large)
                }
            }
        }
        .padding(.horizontal, 20).padding(.top, 12).padding(.bottom, 16)
        .background(.ultraThinMaterial)
    }

    private func choice(_ text: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(text).font(.subheadline.weight(selected ? .semibold : .regular))
                .fixedSize().padding(.horizontal, 13).padding(.vertical, 10)
                .foregroundStyle(selected ? Color.accentColor : .primary)
                .glassEffect(.regular.tint(selected ? Color.accentColor.opacity(0.17) : .clear).interactive(), in: .capsule)
        }.buttonStyle(.plain).accessibilityAddTraits(selected ? [.isSelected] : [])
    }

    private func heading(_ value: String) -> some View { Text(value).font(.subheadline.weight(.semibold)) }
    private func explanation(_ value: String) -> some View { Text(value).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true) }
    private func valueRow(_ title: String, _ value: String) -> some View {
        HStack { Text(title); Spacer(); Text(value).foregroundStyle(.tint) }.font(.subheadline)
    }

    private func changeKind(_ entry: ItemKind) {
        guard kind.family != entry else { return }
        kind = entry
        selectedItem = Self.items(for: entry).first
        tierMatch = .any
        tier = 2
        effectMode = 0
        selectedEffects = []
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
        guard let draft else { return }
        let total = !blanket && !inAlternative && selectedItem != nil && kind == .ring && stackCount > 1 ? stackTotal : nil
        let copies = !blanket && !inAlternative && stackCount > 1 && total == nil ? copyDepth : nil
        onSave(draft, !blanket && !namedOnly ? stackCount : 1, total, copies)
        dismiss()
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
            Slider(value: selection, in: 0...Double(FloorLimits.options.count - (allowsNone ? 0 : 1)), step: 1)
                .accessibilityLabel(title)
                .accessibilityValue(depth.map { "Floor \($0)" } ?? "No limit")
        }
    }
}
