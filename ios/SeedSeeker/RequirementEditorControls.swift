import SwiftUI
import SeedSeekerKit

/// A roomy selection control that keeps the selected option in the same glass
/// surface while its highlight moves between choices.
struct RequirementSegmentedControl<Value: Hashable>: View {
    let title: String
    let options: [(Value, String)]
    @Binding var selection: Value
    @Namespace private var glass
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GlassEffectContainer(spacing: 4) {
            HStack(spacing: 4) {
                ForEach(options, id: \.0) { value, label in
                    Button {
                        withAnimation(reduceMotion ? nil : .spring(response: 0.32, dampingFraction: 0.78)) {
                            selection = value
                        }
                    } label: {
                        Text(label)
                            .font(.subheadline.weight(selection == value ? .semibold : .regular))
                            .foregroundStyle(selection == value ? Color.primary : Color.secondary)
                            .frame(maxWidth: .infinity, minHeight: 44)
                            .contentShape(.capsule)
                            .glassEffect(selection == value ? .regular.tint(Color.accentColor.opacity(0.12)).interactive() : .identity,
                                         in: .capsule)
                            .glassEffectID(selection == value ? "selection" : nil, in: glass)
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(selection == value ? .isSelected : [])
                }
            }
            .padding(4)
            .background(.quaternary.opacity(0.4), in: .capsule)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(title)
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
