// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct ScoutSeedInfoView: View {
    let seed: String
    let mappings: ScoutItemMappings
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var selected: String?
    @Namespace private var lens

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    Text(seed).font(.system(.title3, design: .monospaced)).foregroundStyle(AppTheme.seed)
                    group("Potions", category: "potions", entries: mappings.potions)
                    group("Scrolls", category: "scrolls", entries: mappings.scrolls)
                    group("Rings", category: "rings", entries: mappings.rings)
                }
                .padding(20)
                .accessibilityIdentifier("seed-mappings")
            }
            .background(AppTheme.background)
            .safeAreaBar(edge: .bottom) { caption }
            .sensoryFeedback(.selection, trigger: selected)
            .navigationTitle("Seed information")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Close") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
    }

    /// The identified appearance floats in glass where the thumb already is,
    /// rather than at the top of a scrolled sheet.
    private var caption: some View {
        Text(selected ?? "Tap an appearance to identify it")
            .font(.subheadline.weight(selected == nil ? .regular : .semibold))
            .foregroundStyle(selected == nil ? Color.secondary : Color.primary)
            .multilineTextAlignment(.center)
            .contentTransition(.opacity)
            .textSelection(.enabled)
            .padding(.horizontal, 20).padding(.vertical, 12)
            .frame(minHeight: 48)
            .glassEffect(.regular.tint(selected == nil ? nil : AppTheme.seed.opacity(0.08)), in: .capsule)
            .padding(.horizontal, 20).padding(.bottom, 6)
            .animation(reduceMotion ? nil : .snappy, value: selected)
            .accessibilityIdentifier("mapping-detail")
    }

    private func group(_ title: String, category: String, entries: [ScoutItemMapping]) -> some View {
        let artwork = ItemMappingArtwork.shared
        return VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline).foregroundStyle(AppTheme.seed)
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 3), count: 6), spacing: 3) {
                ForEach(Array(entries.enumerated()), id: \.offset) { index, entry in
                    Button {
                        withAnimation(AppTheme.glassSpring(reduceMotion)) {
                            selected = selected == entry.label ? nil : entry.label
                        }
                    } label: {
                        Group {
                            if let art = artwork.categories[category],
                               let image = SpriteAtlas.bundled?.mappingSprite(entry: entry, art: art,
                                                                              classIndex: index, pointSize: artwork.slotSize * 3) {
                                Image(decorative: image, scale: CGFloat(SpriteAtlas.pixelScale))
                                    .resizable().interpolation(.none).antialiased(false)
                            } else { Color.clear }
                        }
                        .aspectRatio(1, contentMode: .fit)
                        .scaleEffect(selected == entry.label && !reduceMotion ? 1.05 : 1)
                        .background {
                            if selected == entry.label {
                                RoundedRectangle(cornerRadius: 14)
                                    .fill(AppTheme.seed.opacity(0.14))
                                    .overlay { RoundedRectangle(cornerRadius: 14).strokeBorder(AppTheme.seed.opacity(0.4), lineWidth: 1) }
                                    .matchedGeometryEffect(id: "selection", in: lens)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(entry.label)
                    .accessibilityAddTraits(selected == entry.label ? [.isSelected] : [])
                    .accessibilityIdentifier("mapping-\(category)-\(index)")
                }
            }
        }
    }
}
