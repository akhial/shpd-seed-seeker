import SeedSeekerKit
import SwiftUI

struct QueryEstimateView: View {
    let document: Data
    @State private var analyzedDocument: Data?
    @State private var analysis: QueryAnalysis?

    var body: some View {
        let current = analyzedDocument == document ? analysis : nil
        Text(current?.label ?? "Estimating…")
            .font(.caption).monospacedDigit()
            .foregroundStyle(current?.impossible == true ? Color.orange : Color.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal).padding(.top, 10)
            .task(id: document) {
                // Coalesce slider changes and keep engine analysis off the UI thread.
                do { try await Task.sleep(for: .milliseconds(120)) } catch { return }
                let result = await Task.detached(priority: .utility) {
                    try? QueryAnalysis.analyze(document)
                }.value
                guard !Task.isCancelled else { return }
                analysis = result
                analyzedDocument = document
            }
    }
}
