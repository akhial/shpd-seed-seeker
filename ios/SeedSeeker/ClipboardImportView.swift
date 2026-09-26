import SwiftUI

/// The system paste control supplies user-authorized text even when direct
/// cross-app pasteboard reads are denied. Never reread the pasteboard here.
struct ClipboardImportView: View {
    @Environment(\.dismiss) private var dismiss
    let onPaste: (String) -> Void

    var body: some View {
        NavigationStack {
            PasteButton(payloadType: String.self) { values in
                onPaste(values.first ?? "")
            }
            .controlSize(.large)
            .buttonStyle(.glassProminent)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .navigationTitle("Import from clipboard")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
        .presentationDetents([.height(220)])
        .presentationDragIndicator(.visible)
    }
}
