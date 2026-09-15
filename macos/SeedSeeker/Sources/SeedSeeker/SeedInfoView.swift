// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct SeedInfoView: View {
    let seed: String
    let mappings: ScoutItemMappings
    @Environment(\.dismiss) private var dismiss
    @State private var selected: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Seed information").font(.headline)
                Spacer()
                Button("Close") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            Text(seed).font(.system(.title3, design: .monospaced))
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    if let selected { Text(selected).font(.caption).textSelection(.enabled) }
                    group("Potions", category: "potions", entries: mappings.potions)
                    group("Scrolls", category: "scrolls", entries: mappings.scrolls)
                    group("Rings", category: "rings", entries: mappings.rings)
                }
            }
        }
        .padding(24)
        .frame(width: 369, height: 560)
    }

    private func group(_ title: String, category: String, entries: [ScoutItemMapping]) -> some View {
        let artwork = ItemMappingArtwork.shared
        let size = artwork.slotSize * 3
        let gap = CGFloat(artwork.slotGap * 3)
        return VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.headline)
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(CGFloat(size)), spacing: gap), count: 6), spacing: gap) {
                ForEach(Array(entries.enumerated()), id: \.offset) { index, entry in
                    Button {
                        selected = selected == entry.label ? nil : entry.label
                    } label: {
                        Group {
                            if let art = artwork.categories[category],
                               let image = SpriteAtlas.bundled?.mappingSprite(entry: entry, art: art, classIndex: index, pointSize: size) {
                                Image(decorative: image, scale: CGFloat(SpriteAtlas.pixelScale))
                                    .resizable().interpolation(.none)
                            } else {
                                Image(systemName: "questionmark").foregroundStyle(.secondary)
                            }
                        }
                        .frame(width: CGFloat(size), height: CGFloat(size))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(entry.label)
                    .accessibilityLabel(entry.label)
                }
            }
        }
    }
}
