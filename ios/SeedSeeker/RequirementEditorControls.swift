import SwiftUI
import SeedSeekerKit
import UIKit

/// Retains the system's draggable glass selection lens, with room for the
/// longer enchantment labels and a comfortable touch target.
struct RequirementSegmentedControl<Value: Hashable>: View {
    let title: String
    let options: [(Value, String)]
    @Binding var selection: Value
    @ScaledMetric(relativeTo: .subheadline) private var height = 52

    var body: some View {
        RequirementNativeSegments(title: title, options: options, selection: $selection)
            .frame(height: max(52, height))
    }
}

private struct RequirementNativeSegments<Value: Hashable>: UIViewRepresentable {
    let title: String
    let options: [(Value, String)]
    @Binding var selection: Value

    func makeCoordinator() -> Coordinator { Coordinator(selection: $selection, options: options) }

    func makeUIView(context: Context) -> UISegmentedControl {
        let control = UISegmentedControl(items: options.map(\.1))
        control.apportionsSegmentWidthsByContent = true
        control.setContentHuggingPriority(.defaultLow, for: .horizontal)
        control.addTarget(context.coordinator, action: #selector(Coordinator.select(_:)), for: .valueChanged)
        return control
    }

    func updateUIView(_ control: UISegmentedControl, context: Context) {
        context.coordinator.selection = $selection
        context.coordinator.options = options
        if control.numberOfSegments != options.count {
            control.removeAllSegments()
            for (index, option) in options.enumerated() {
                control.insertSegment(withTitle: option.1, at: index, animated: false)
            }
        } else {
            for (index, option) in options.enumerated() where control.titleForSegment(at: index) != option.1 {
                control.setTitle(option.1, forSegmentAt: index)
            }
        }
        let index = options.firstIndex { $0.0 == selection } ?? UISegmentedControl.noSegment
        if control.selectedSegmentIndex != index { control.selectedSegmentIndex = index }
        control.accessibilityLabel = title
        control.tintColor = UIColor(AppTheme.accent)
        control.selectedSegmentTintColor = UIColor(AppTheme.accent).withAlphaComponent(0.22)
        let font = UIFont.preferredFont(forTextStyle: .subheadline, compatibleWith: control.traitCollection)
        control.setTitleTextAttributes([.font: font, .foregroundColor: UIColor.secondaryLabel], for: .normal)
        control.setTitleTextAttributes([.font: UIFont.systemFont(ofSize: font.pointSize, weight: .semibold),
                                        .foregroundColor: UIColor(AppTheme.accent)], for: .selected)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: UISegmentedControl, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? uiView.intrinsicContentSize.width, height: proposal.height ?? 52)
    }

    @MainActor final class Coordinator: NSObject {
        var selection: Binding<Value>
        var options: [(Value, String)]

        init(selection: Binding<Value>, options: [(Value, String)]) {
            self.selection = selection
            self.options = options
        }

        @objc func select(_ control: UISegmentedControl) {
            guard options.indices.contains(control.selectedSegmentIndex) else { return }
            selection.wrappedValue = options[control.selectedSegmentIndex].0
        }
    }
}

/// Native stepped sliders with actual tick marks, so touch, VoiceOver and
/// keyboard adjustment all retain the system slider behavior.
struct RequirementGraduatedSlider: View {
    let title: String
    @Binding var value: Double
    let bounds: ClosedRange<Double>

    var body: some View {
        Slider(value: $value, in: bounds, step: 1, label: { Text(title) },
               tick: { SliderTick($0) })
            .frame(minHeight: 44)
            .accessibilityLabel(title)
    }
}

struct RequirementSourceSelector: View {
    var title = "Source"
    @Binding var source: ScoutItemSource?

    var body: some View {
        Menu {
            Picker(title, selection: $source) {
                Text("Any source").tag(ScoutItemSource?.none)
                ForEach(ScoutItemSource.allCases, id: \.rawValue) { option in
                    Text(option.label).tag(ScoutItemSource?.some(option))
                }
            }
        } label: {
            HStack(spacing: 12) {
                Text(title).foregroundStyle(.primary)
                Spacer(minLength: 8)
                Text(source?.label ?? "Any source")
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.trailing)
                Image(systemName: "chevron.up.chevron.down")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            .font(.subheadline)
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .contentShape(.rect(cornerRadius: 18))
            .glassEffect(.regular.interactive(), in: .rect(cornerRadius: 18))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityValue(source?.label ?? "Any source")
    }
}
