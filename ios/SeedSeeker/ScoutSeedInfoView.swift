// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct ScoutSeedInfoView: View {
    let seed: String
    let mappings: ScoutItemMappings
    @Environment(\.dismiss) private var dismiss
    @State private var selected: String?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    Text(seed).font(.system(.title3, design: .monospaced)).foregroundStyle(AppTheme.seed)
                    if let selected {
                        Text(selected).font(.subheadline).textSelection(.enabled)
                            .accessibilityIdentifier("mapping-detail")
                    }
                    group("Potions", category: "potions", entries: mappings.potions)
                    group("Scrolls", category: "scrolls", entries: mappings.scrolls)
                    group("Rings", category: "rings", entries: mappings.rings)
                }
                .padding(20)
                .accessibilityIdentifier("seed-mappings")
            }
            .background(AppTheme.background)
            .navigationTitle("Seed information")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Close") { dismiss() } } }
        }
        .presentationDetents([.medium, .large])
    }

    private func group(_ title: String, category: String, entries: [ScoutItemMapping]) -> some View {
        let artwork = ItemMappingArtwork.shared
        return VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline).foregroundStyle(AppTheme.seed)
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 3), count: 6), spacing: 3) {
                ForEach(Array(entries.enumerated()), id: \.offset) { index, entry in
                    Button { selected = selected == entry.label ? nil : entry.label } label: {
                        Group {
                            if let art = artwork.categories[category],
                               let image = SpriteAtlas.bundled?.mappingSprite(entry: entry, art: art,
                                                                              classIndex: index, pointSize: artwork.slotSize * 3) {
                                Image(decorative: image, scale: CGFloat(SpriteAtlas.pixelScale))
                                    .resizable().interpolation(.none).antialiased(false)
                            } else { Color.clear }
                        }
                        .aspectRatio(1, contentMode: .fit)
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
