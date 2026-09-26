import SwiftUI
import SeedSeekerKit

/// Shared geometry lets the board's lifted chips reach the fixed Finder action
/// without putting a scrolling drop target underneath the user's finger.
@MainActor @Observable
final class RequirementBoardInteraction {
    var activeID: String?
    var location = CGPoint.zero
    var removeFrame = CGRect.zero
    var lift: RequirementLift?
    var preview: AnyView?

    var isDragging: Bool { activeID != nil }
    var isOverRemove: Bool {
        isDragging && !removeFrame.isEmpty && removeFrame.insetBy(dx: -18, dy: -18).contains(location)
    }

    func reset() {
        activeID = nil
        location = .zero
        preview = nil
    }
}

struct RequirementLift {
    let id: String
    let frame: CGRect
    let requirement: ItemRequirement?
    let item: BoardItem?
    var translation = CGSize.zero
    var scale: CGFloat = 1.06
    var opacity: Double = 1
}

/// Render above the entire Finder, including settings, results and its fixed
/// action. A lift belongs to the landscape rather than the scrolling row.
struct RequirementsLiftOverlay: View {
    let interaction: RequirementBoardInteraction
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { geometry in
            if let lift = interaction.lift, let preview = interaction.preview {
                let origin = geometry.frame(in: .global).origin
                let halfWidth = min(geometry.size.width / 2, lift.frame.width * lift.scale / 2 + 12)
                let horizontalPosition = min(geometry.size.width - halfWidth,
                    max(halfWidth, lift.frame.midX + lift.translation.width - origin.x))
                preview
                    .frame(width: lift.frame.width, height: lift.frame.height)
                    .glassEffect(.regular.tint(interaction.isOverRemove ? .red.opacity(0.16) : .white.opacity(0.07)), in: .capsule)
                    .shadow(color: .black.opacity(0.3), radius: 16, y: 12)
                    .scaleEffect(x: lift.scale * (!reduceMotion && interaction.isOverRemove ? 0.76 : 1),
                                 y: lift.scale * (!reduceMotion && interaction.isOverRemove ? 1.08 : 1))
                    .rotationEffect(.degrees(reduceMotion || interaction.isOverRemove ? 0 : min(4, max(-4, Double(lift.translation.width / 70)))))
                    .opacity(lift.opacity)
                    .position(x: horizontalPosition,
                              y: lift.frame.midY + lift.translation.height - origin.y - (reduceMotion ? 0 : 10))
                    .animation(reduceMotion ? nil : .spring(response: 0.42, dampingFraction: 0.76), value: interaction.isOverRemove)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

struct RequirementsRemoveTarget: View {
    let interaction: RequirementBoardInteraction
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Label("Drop to remove", systemImage: interaction.isOverRemove ? "trash.fill" : "trash")
            .font(.body.weight(.semibold))
            .foregroundStyle(.red)
            .fixedSize()
            .frame(width: !reduceMotion && interaction.isOverRemove ? 252 : 228,
                   height: !reduceMotion && interaction.isOverRemove ? 62 : 54,
                   alignment: .center)
            .glassEffect(.regular.tint(.red.opacity(interaction.isOverRemove ? 0.3 : 0.1)), in: .capsule)
            .frame(height: 62, alignment: .center)
            .symbolEffect(.bounce, value: !reduceMotion && interaction.isOverRemove)
            .onGeometryChange(for: CGRect.self) { $0.frame(in: .global) } action: {
                interaction.removeFrame = $0
            }
            .animation(reduceMotion ? nil : .spring(response: 0.3, dampingFraction: 0.6), value: interaction.isOverRemove)
            .accessibilityAddTraits(.isButton)
    }
}
