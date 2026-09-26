// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct ScoutItemCard: View {
    let item: ScoutItem
    let ringGems: RingGems
    let matched: Bool
    let dimmed: Bool
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            ItemSpriteView(item: item.item, ringGems: ringGems, glow: itemGlow(item), pointSize: 36)
            VStack(alignment: .leading, spacing: 5) {
                RequirementsFlowLayout(spacing: 6) {
                    Text(item.item.name).font(.subheadline.weight(.semibold))
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1).minimumScaleFactor(0.8)
                    if item.displayedUpgrade != 0 {
                        ScoutBadge(text: "+\(item.displayedUpgrade)", color: AppTheme.upgrade, weight: .bold)
                    }
                    if item.cursed { ScoutBadge(text: "cursed", color: ScoutItemColors.curse, weight: .bold) }
                    if item.secret { ScoutBadge(text: "secret", color: ScoutItemColors.secret, weight: .bold) }
                }
                RequirementsFlowLayout(spacing: 6) {
                    if let effect = item.effect {
                        Text(effect).foregroundStyle(ItemCatalog.cursesFor(item.item.kind).contains(effect) ? ScoutItemColors.curseEffect : AppTheme.teal)
                    }
                    Text(item.source.label).foregroundStyle(.secondary)
                }.font(.caption)
                if case let .scenarios(group, _) = item.accessibility {
                    Text("Route group \(ScoutChoiceStatus.letter(group)) · access changes with room choices")
                        .font(.caption2).foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if matched || hasChoice {
                VStack(alignment: .trailing, spacing: 4) {
                    if matched {
                        Label("match", systemImage: "checkmark").font(.caption2.weight(.semibold))
                            .foregroundStyle(AppTheme.softGreen).padding(.horizontal, 6).padding(.vertical, 3)
                            .background(AppTheme.softGreen.opacity(0.12), in: Capsule())
                    }
                    ScoutChoiceBadge(accessibility: item.accessibility)
                }
                .fixedSize()
            }
        }
        .padding(14)
        .background(matched ? AppTheme.raised : AppTheme.surface, in: RoundedRectangle(cornerRadius: 18))
        .opacity(dimmed ? 0.45 : 1)
        .accessibilityElement(children: .combine)
    }

    private var hasChoice: Bool {
        if case .choice = item.accessibility { true } else { false }
    }
}

private enum ScoutItemColors {
    static let curse = Color(red: 242 / 255, green: 149 / 255, blue: 138 / 255)
    static let curseEffect = Color(red: 217 / 255, green: 108 / 255, blue: 95 / 255)
    static let secret = Color(red: 201 / 255, green: 166 / 255, blue: 245 / 255)
}

struct ScoutChoiceBadge: View {
    let accessibility: ScoutAccessibility
    var body: some View {
        if case let .choice(group, option) = accessibility {
            Label(ScoutChoiceStatus.letter(group), systemImage: "arrow.triangle.branch")
                .font(.caption2).foregroundStyle(.secondary)
                .padding(.horizontal, 6).padding(.vertical, 3)
                .background(.secondary.opacity(0.12), in: RoundedRectangle(cornerRadius: 5))
                .accessibilityLabel("One reward of choice group \(ScoutChoiceStatus.letter(group)) (option \(option + 1))")
        }
    }
}

struct ScoutTrinketShortcuts: View {
    let world: ScoutWorld
    let enabled: Bool
    let onSelect: (String) -> Void
    var body: some View {
        HStack(spacing: 4) {
            ForEach(Array(world.trinketOrder.prefix(4))) { item in
                let applied = world.selectedTrinket == item.id
                Button { onSelect(applied ? "none" : item.id) } label: {
                    ItemSpriteView(item: item, pointSize: 22)
                        .frame(width: 34, height: 34)
                        .background(applied ? AppTheme.accent.opacity(0.14) : AppTheme.surface,
                                    in: RoundedRectangle(cornerRadius: 9))
                        .overlay(RoundedRectangle(cornerRadius: 9)
                            .strokeBorder(applied ? AppTheme.accent : .secondary.opacity(0.25), lineWidth: applied ? 2 : 1))
                }
                .buttonStyle(.plain)
                .disabled(!enabled)
                .accessibilityLabel(item.name)
                .accessibilityValue(applied ? "Applied +3" : "Not applied")
            }
        }
    }
}

struct ScoutTrinketCard: View {
    let world: ScoutWorld
    let choices: [(offset: Int, element: ScoutItem)]
    let matches: ScoutMatches?
    let enabled: Bool
    let onSelect: (String) -> Void

    private let catalyst = CatalogItem(id: "trinket_catalyst", name: "Magical catalyst", kind: .trinket, spriteIndex: 70)
    private var offered: [CatalogItem] {
        world.trinketOrder.isEmpty ? choices.map(\.element.item) : Array(world.trinketOrder.prefix(4))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let placement = choices.first?.element {
                HStack(spacing: 10) {
                    ItemSpriteView(item: catalyst, pointSize: 36)
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Magical catalyst").font(.subheadline.weight(.semibold))
                        Text(placement.source.label).font(.caption).foregroundStyle(.secondary)
                        if placement.secret { Text("Secret room").font(.caption2).foregroundStyle(.purple) }
                        if case let .scenarios(group, _) = placement.accessibility {
                            Text("Route group \(ScoutChoiceStatus.letter(group)) · access changes with room choices")
                                .font(.caption2).foregroundStyle(.secondary)
                        }
                    }
                    Spacer(minLength: 0)
                    ScoutChoiceBadge(accessibility: placement.accessibility)
                }
            }
            HStack(spacing: 6) {
                ForEach(offered) { trinket in
                    let applied = world.selectedTrinket == trinket.id
                    let matched = choices.contains { $0.element.item.id == trinket.id && matches?.matched.contains($0.offset) == true }
                    Button { onSelect(applied ? "none" : trinket.id) } label: {
                        VStack(spacing: 3) {
                            if applied { Text("Applied +3").font(.system(size: 10, weight: .bold)).foregroundStyle(AppTheme.upgrade) }
                            ItemSpriteView(item: trinket, pointSize: applied ? 32 : 40)
                            Text(trinket.name).font(.system(size: 11)).lineLimit(1).minimumScaleFactor(0.25)
                                .foregroundStyle(.primary)
                        }
                        .padding(5)
                        .frame(maxWidth: .infinity, minHeight: 80, maxHeight: 80)
                        .background(matched ? AppTheme.accent.opacity(0.14) : AppTheme.raised,
                                    in: RoundedRectangle(cornerRadius: 12))
                        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(
                            applied ? AppTheme.accent : .clear, lineWidth: 2))
                    }
                    .buttonStyle(.plain).disabled(!enabled)
                    .accessibilityLabel(trinket.name + (matched ? ", matches requirement" : ""))
                    .accessibilityValue(applied ? "Applied +3" : "Not applied")
                }
            }
            if world.trinketOrder.count > 4 {
                Text("Transmutation order · 1–13").font(.caption2).foregroundStyle(.secondary)
                HStack(spacing: 2) {
                    ForEach(Array(world.trinketOrder.dropFirst(4).enumerated()), id: \.element.id) { index, trinket in
                        let matched = matches?.transmutedTrinkets.contains(index) == true
                        ItemSpriteView(item: trinket, pointSize: 20,
                                       label: "Transmutation #\(index + 1): \(trinket.name)" + (matched ? ", matches requirement" : ""))
                            .frame(maxWidth: .infinity, minHeight: 24)
                            .background(matched ? AppTheme.accent.opacity(0.14) : .clear, in: RoundedRectangle(cornerRadius: 4))
                            .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(matched ? AppTheme.accent : .clear))
                    }
                }
            }
        }
        .padding(12)
        .background(AppTheme.surface, in: RoundedRectangle(cornerRadius: 18))
    }
}
