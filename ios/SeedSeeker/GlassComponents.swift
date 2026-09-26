// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI
import UIKit

extension AppTheme {
    /// Every glass morph shares one spring, so shapes that split, merge and
    /// grow in the same gesture move as a single material.
    static func glassSpring(_ reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : .spring(response: 0.42, dampingFraction: 0.8)
    }

    /// A glow as seen through dark glass. The game's darkest glows (Grim,
    /// Stone, Vampiric…) would vanish on this background, so they are lifted
    /// toward white by how dark they are while keeping their hue.
    static func glowDisplayColor(_ glow: ItemGlow) -> Color {
        let (red, green, blue) = glow.components
        let luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue
        let lift = luminance < 0.4 ? (0.4 - luminance) * 1.2 : 0
        return Color(.sRGB, red: red + (1 - red) * lift, green: green + (1 - green) * lift,
                     blue: blue + (1 - blue) * lift)
    }

    static let curse = Color(red: 217 / 255, green: 108 / 255, blue: 95 / 255)
}

extension View {
    /// Every choice keeps its own glass, so a GlassEffectContainer can melt
    /// choices into one another as rows change. The selection is a tinted lens
    /// inside that glass which glides to the new choice rather than re-tinting
    /// in place.
    func glassChoice<Value: Hashable & Sendable, S: Shape>(
        _ value: Value, selected: Bool, lens: String = "lens", in namespace: Namespace.ID,
        tint: Color = AppTheme.accent, shape: S
    ) -> some View {
        background {
            if selected {
                shape.fill(tint.opacity(0.34))
                    .overlay { shape.stroke(tint.opacity(0.35), lineWidth: 0.5) }
                    .matchedGeometryEffect(id: lens, in: namespace)
            }
        }
        .glassEffect(.regular.interactive(), in: shape)
        .glassEffectID(value, in: namespace)
    }

    func glassChoice<Value: Hashable & Sendable>(
        _ value: Value, selected: Bool, lens: String = "lens", in namespace: Namespace.ID,
        tint: Color = AppTheme.accent
    ) -> some View {
        glassChoice(value, selected: selected, lens: lens, in: namespace, tint: tint, shape: .capsule)
    }
}

/// Copy confirms in place: the glass takes on the accent and the symbol
/// morphs into a checkmark, then settles back.
struct GlassCopyButton: View {
    let text: String
    let label: String
    var size: CGFloat = 40
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var copied = false
    @State private var reset: Task<Void, Never>?

    var body: some View {
        Button {
            UIPasteboard.general.string = text
            withAnimation(AppTheme.glassSpring(reduceMotion)) { copied = true }
            reset?.cancel()
            reset = Task { @MainActor in
                try? await Task.sleep(for: .seconds(1.4))
                guard !Task.isCancelled else { return }
                withAnimation(AppTheme.glassSpring(reduceMotion)) { copied = false }
            }
        } label: {
            Image(systemName: copied ? "checkmark" : "doc.on.doc")
                .font(.system(size: size * 0.4, weight: .semibold))
                .foregroundStyle(copied ? AppTheme.upgrade : Color.primary.opacity(0.8))
                .contentTransition(.symbolEffect(.replace))
                .frame(width: size, height: size)
                .glassEffect(.regular.tint(copied ? AppTheme.accent.opacity(0.3) : nil).interactive(), in: .circle)
        }
        .buttonStyle(.plain)
        .sensoryFeedback(.success, trigger: copied) { _, now in now }
        .accessibilityLabel(label)
        .accessibilityValue(copied ? "Copied" : "")
        .onDisappear { reset?.cancel() }
    }
}

/// The four starting trinkets share one lens: applying another slides the
/// lens across rather than moving a border.
struct TrinketGlassShortcuts: View {
    let world: ScoutWorld
    let enabled: Bool
    var size: CGFloat = 38
    let onSelect: (String) -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Namespace private var lens

    var body: some View {
        GlassEffectContainer(spacing: 6) {
            HStack(spacing: 6) {
                ForEach(Array(world.trinketOrder.prefix(4))) { item in
                    let applied = world.selectedTrinket == item.id
                    Button { onSelect(applied ? "none" : item.id) } label: {
                        ItemSpriteView(item: item, pointSize: Int(size * 0.58))
                            .scaleEffect(applied && !reduceMotion ? 1.12 : 1)
                            .frame(width: size, height: size)
                            .glassChoice(item.id, selected: applied, in: lens,
                                         shape: .rect(cornerRadius: size * 0.32))
                    }
                    .buttonStyle(.plain)
                    .disabled(!enabled)
                    .opacity(enabled ? 1 : 0.55)
                    .accessibilityLabel(item.name)
                    .accessibilityValue(applied ? "Applied +3" : "Not applied")
                    .accessibilityAddTraits(applied ? [.isSelected] : [])
                }
            }
        }
        .animation(AppTheme.glassSpring(reduceMotion), value: world.selectedTrinket)
        .sensoryFeedback(.selection, trigger: world.selectedTrinket)
    }
}

/// Light travelling through the glass while work is under way. It is a
/// highlight band clipped to the host's shape, never a hit target.
struct GlassSheen<S: Shape>: View {
    let shape: S
    var period: Double = 2.4
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var sweep = false

    var body: some View {
        GeometryReader { geometry in
            let band = max(60, geometry.size.width * 0.4)
            LinearGradient(stops: [.init(color: .clear, location: 0),
                                   .init(color: .white.opacity(0.13), location: 0.5),
                                   .init(color: .clear, location: 1)],
                           startPoint: .leading, endPoint: .trailing)
                .frame(width: band)
                .rotationEffect(.degrees(12))
                .offset(x: sweep ? geometry.size.width + band : -band * 1.5)
        }
        .clipShape(shape)
        .blendMode(.plusLighter)
        .opacity(reduceMotion ? 0 : 1)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: period).repeatForever(autoreverses: false)) { sweep = true }
        }
    }
}
