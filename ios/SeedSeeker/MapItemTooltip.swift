// SPDX-License-Identifier: GPL-3.0-or-later
import Observation
import SeedSeekerKit
import SwiftUI
import UIKit

@MainActor @Observable final class MapItemPresentation {
    struct Card {
        let tip: LevelMapDocument.ItemTooltip
        let frame: CGRect
    }

    private(set) var card: Card?

    func show(_ card: Card?, animated: Bool) {
        if animated {
            withAnimation { self.card = card }
        } else {
            var transaction = Transaction()
            transaction.disablesAnimations = true
            withTransaction(transaction) { self.card = card }
        }
    }
}

/// The container stays mounted through dismissal so Liquid Glass owns both
/// transitions. Outside the card, the map keeps receiving taps and gestures.
@MainActor final class MapItemOverlayHost: UIView {
    let presentation = MapItemPresentation()

    init(frame: CGRect, onClose: @escaping () -> Void) {
        super.init(frame: frame)
        backgroundColor = .clear
        let presentation = self.presentation
        let content = UIHostingConfiguration {
            MapItemOverlay(presentation: presentation, onClose: onClose)
        }.margins(.all, 0).makeContentView()
        content.frame = bounds
        content.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        content.backgroundColor = .clear
        addSubview(content)
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        guard let frame = presentation.card?.frame, frame.contains(point) else { return nil }
        return super.hitTest(point, with: event)
    }

    override func accessibilityPerformEscape() -> Bool {
        superview?.accessibilityPerformEscape() ?? false
    }
}

private struct MapItemOverlay: View {
    let presentation: MapItemPresentation
    let onClose: () -> Void
    @Namespace private var glassNamespace

    var body: some View {
        GlassEffectContainer {
            ZStack(alignment: .topLeading) {
                if let card = presentation.card {
                    VStack(spacing: 0) {
                        MapItemCard(tip: card.tip, onClose: onClose)
                            .header
                            .frame(maxHeight: min(52, card.frame.height), alignment: .top)
                        ScrollView {
                            MapItemCard(tip: card.tip).details
                        }
                        .scrollBounceBehavior(.basedOnSize)
                    }
                    .frame(width: card.frame.width, height: card.frame.height)
                    .clipShape(.rect(cornerRadius: 20))
                    .glassEffect(.regular.tint(AppTheme.surface.opacity(0.35)), in: .rect(cornerRadius: 20))
                    .glassEffectID(card.tip.cell, in: glassNamespace)
                    .glassEffectTransition(.materialize)
                    .offset(x: card.frame.minX, y: card.frame.minY)
                    .accessibilityIdentifier("map-item-tooltip")
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
    }
}

struct MapItemCard: View {
    let tip: LevelMapDocument.ItemTooltip
    var onClose: () -> Void = {}

    var body: some View {
        VStack(spacing: 0) {
            header
            details
        }
    }

    var header: some View {
        HStack {
            if !tip.label.isEmpty {
                Text(tip.label)
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
            Button("Close", systemImage: "xmark", action: onClose)
                .labelStyle(.iconOnly)
                .font(.caption.weight(.semibold))
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
        }
        .padding(.horizontal, 14).padding(.top, 10).padding(.bottom, 8)
    }

    var details: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(tip.items.enumerated()), id: \.offset) { index, item in
                if index > 0 { Divider() }
                MapItemHeading(item: item)
                if item.cursed == true || item.curse != nil {
                    Text(item.cursed == true ? "Cursed" : "Curse")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.red)
                        .padding(.horizontal, 7).padding(.vertical, 3)
                        .background(.red.opacity(0.12), in: .rect(cornerRadius: 6))
                }
                if !item.deterministic {
                    Text("Varies with play").font(.caption).foregroundStyle(.secondary)
                }
                if !item.description.isEmpty {
                    Text(item.description)
                        .font(.callout)
                        .foregroundStyle(.primary.opacity(0.85))
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .padding(.horizontal, 14).padding(.bottom, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct MapItemHeading: View {
    let item: LevelMapDocument.TooltipItem
    @ScaledMetric(relativeTo: .caption) private var upgradeFontSize = 12

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            MapItemSpriteView(item: item)
            title
                .font(.headline.bold())
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityLabel(item.name + (item.upgrade.flatMap { $0 > 0 ? " +\($0)" : nil } ?? ""))
            if item.quantity > 1 {
                Spacer(minLength: 0)
                Text("×\(item.quantity)").font(.subheadline).foregroundStyle(.secondary).fixedSize()
            }
        }
    }

    private var title: Text {
        guard let upgrade = item.upgrade, upgrade > 0 else { return Text(item.name) }
        let font = UIFont.monospacedSystemFont(ofSize: upgradeFontSize, weight: .bold)
        let text = "+\(upgrade)" as NSString
        let color = UIColor(AppTheme.upgrade)
        let attributes: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
        let size = CGSize(width: ceil(text.size(withAttributes: attributes).width) + 8,
                          height: ceil(font.lineHeight) + 4)
        let badge = UIGraphicsImageRenderer(size: size).image { _ in
            color.withAlphaComponent(0.12).setFill()
            UIBezierPath(roundedRect: CGRect(origin: .zero, size: size), cornerRadius: 4).fill()
            text.draw(at: CGPoint(x: 4, y: 2), withAttributes: attributes)
        }
        // Keep the upgrade chip beside the final word when the name wraps.
        return Text("\(Text(item.name))\u{00a0}\(Text(Image(uiImage: badge)).baselineOffset(font.descender - 2))")
    }
}
