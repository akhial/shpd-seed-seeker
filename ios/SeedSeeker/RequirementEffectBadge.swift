import SeedSeekerKit
import SwiftUI

/// Resolve the full selected set in catalog order, as the web's effectGlows
/// does. "Any enchantment" has no fixed sprite colour of its own.
func requirementGlows(_ effect: EffectFilter) -> [ItemGlow] {
    effect.names.map { enchantmentGlows[$0] ?? curseGlow }
}

/// The web's effect wheel is stationary: all selected colours remain visible
/// around the count while the item's sprite pulses through them in turn.
struct RequirementEffectBadge: View {
    let effect: EffectFilter
    let isWildcard: Bool

    @ViewBuilder var body: some View {
        let glows = requirementGlows(effect)
        if glows.count > 1 {
            let colors = glows.map(color)
            Text("\(glows.count)")
                .font(.system(size: 10, weight: .bold, design: .monospaced))
                .foregroundStyle(.primary)
                .frame(width: 20, height: 20)
                .background(AppTheme.background, in: .circle)
                .overlay {
                    Circle().strokeBorder(
                        AngularGradient(colors: colors + [colors[0]], center: .center,
                                        startAngle: .degrees(-90), endAngle: .degrees(270)),
                        lineWidth: 2.5)
                }
                .accessibilityHidden(true)
        } else if effect == .anyEnchantment {
            Circle()
                .fill(AngularGradient(colors: Self.spectrum, center: .center,
                                      startAngle: .degrees(-90), endAngle: .degrees(270)))
                .frame(width: 12, height: 12)
                .accessibilityHidden(true)
        } else if isWildcard, let glow = glows.first {
            Circle()
                .fill(color(glow))
                .frame(width: 12, height: 12)
                .shadow(color: color(glow), radius: 2.5)
                .accessibilityHidden(true)
        }
    }

    private func color(_ glow: ItemGlow) -> Color {
        let (red, green, blue) = glow.components
        return Color(.sRGB, red: red, green: green, blue: blue)
    }

    private static let spectrum: [Color] = [
        Color(red: 1, green: 1 / 3, blue: 1 / 3),
        Color(red: 1, green: 1, blue: 1 / 3),
        Color(red: 1 / 3, green: 1, blue: 1 / 3),
        Color(red: 1 / 3, green: 1, blue: 1),
        Color(red: 1 / 3, green: 1 / 3, blue: 1),
        Color(red: 1, green: 1 / 3, blue: 1),
        Color(red: 1, green: 1 / 3, blue: 1 / 3),
    ]
}
