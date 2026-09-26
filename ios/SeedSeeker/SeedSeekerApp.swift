// SPDX-License-Identifier: GPL-3.0-or-later
import SwiftUI
import SeedSeekerKit
import UniformTypeIdentifiers
import UIKit

enum AppTheme {
    static let background = Color(red: 27/255, green: 29/255, blue: 35/255)
    static let surface = Color(red: 35/255, green: 38/255, blue: 47/255)
    static let raised = Color(red: 38/255, green: 42/255, blue: 51/255)
    static let accent = Color(red: 86/255, green: 189/255, blue: 82/255)
    static let upgrade = Color(red: 131/255, green: 252/255, blue: 100/255)
    static let softGreen = Color(red: 110/255, green: 201/255, blue: 143/255)
    static let seed = Color(red: 1, green: 1, blue: 85/255)
    static let teal = Color(red: 88/255, green: 194/255, blue: 180/255)
}

@main
struct SeedSeekerApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            SeedSeekerRootView(model: model)
                .tint(AppTheme.accent)
                .preferredColorScheme(.dark)
        }
    }
}

enum AppTab: Hashable { case finder, scout }
enum AppSheet: String, Identifiable {
    case settings, searchSettings, about, presets
    var id: String { rawValue }
}

@MainActor @Observable
final class AppModel {
    var query: SavedQuery {
        didSet {
            if let text = QueryPersistence.encode(query) { defaults.set(text, forKey: "savedQuery") }
            showResults = false
        }
    }
    var presets: [QueryPreset] {
        didSet {
            if let text = PresetPersistence.encode(presets) { defaults.set(text, forKey: "savedPresets") }
        }
    }
    let controller: SearchController
    let backgroundSearch: BackgroundSearch
    let scout = ScoutModel()
    var tab: AppTab = .finder
    var showResults = false
    var sheet: AppSheet?
    var importNotice: String?
    var alertTitle = "Results transfer"
    var alertMessage: String?
    var sharedLink: SharePayload?
    var exportDocument: ResultsDocument?
    var showingImporter = false
    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let support = URL.applicationSupportDirectory.appendingPathComponent("SeedSeeker", isDirectory: true)
        controller = SearchController(checkpointURL: support.appendingPathComponent("search.json"))
        backgroundSearch = BackgroundSearch(controller: controller)
        if let saved = defaults.string(forKey: "savedQuery") {
            query = QueryPersistence.decode(saved)
        } else {
            query = SavedQuery(requirements: [try! ItemRequirement(key: 1,
                item: ItemCatalog.findById("wand_fireblast"), upgrade: 3, kind: .wand)])
        }
        if let pending = controller.pendingQuery { query = pending }
        presets = PresetPersistence.decode(defaults.string(forKey: "savedPresets") ?? "")
        showResults = controller.hasPendingSearch || !controller.results.isEmpty
    }

    var request: SearchRequest? { try? query.searchRequest() }
    var validationMessage: String? { AndroidCopy.validationMessage(for: query) }

    func search() {
        guard let request else { return }
        importNotice = nil
        controller.start(request, workers: defaults.integer(forKey: WorkerPersistence.defaultsKey))
        backgroundSearch.start()
        showResults = true
    }

    func scoutResult(_ seed: String) {
        let result = controller.results.first { $0.seed == seed }
        let request = result == nil ? self.request : (try? controller.exportQuery?.searchRequest()) ?? self.request
        controller.selectedSeed = seed
        scout.scout(seed: seed, query: request, result: result, challenges: query.challenges)
        tab = .scout
    }

    func share() {
        do { sharedLink = SharePayload(text: try DeepLink.encodeLink(for: query)) }
        catch { showError(AndroidCopy.shareError(error), title: "Shared search") }
    }

    func open(_ url: URL) {
        if url.isFileURL { importFile(url); return }
        guard url.scheme?.lowercased() == DeepLink.scheme || url.fragment?.hasPrefix("q=") == true
                || URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.contains(where: { $0.name == "q" }) == true
        else { return }
        guard !controller.isRunning else {
            showError("Stop the search before opening a shared search.", title: "Shared search"); return
        }
        do {
            query = try DeepLink.decode(url.absoluteString)
            controller.clearDisplayedResults()
            importNotice = "Loaded shared search"
            tab = .finder
        } catch { showError(AndroidCopy.linkError(error), title: "Shared search") }
    }

    func importClipboard() {
        guard let text = UIPasteboard.general.string, !text.isEmpty else {
            showError("The clipboard has no text. Copy results JSON and try again."); return
        }
        importText(text, source: "clipboard")
    }

    func importFile(_ url: URL) {
        importResults(source: "file") {
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            guard let handle = try? FileHandle(forReadingFrom: url) else {
                throw ResultsExportError("Could not read the selected file.")
            }
            defer { try? handle.close() }
            let limit = EngineInfo.shared.resultsFileMaxBytes + 1
            var data = Data()
            while data.count < limit {
                guard let chunk = try handle.read(upToCount: min(65_536, limit - data.count)), !chunk.isEmpty else { break }
                data.append(chunk)
            }
            return String(decoding: data, as: UTF8.self)
        }
    }

    func importText(_ text: String, source: String) {
        importResults(source: source) { text }
    }

    private func importResults(source: String, read: @escaping @Sendable () throws -> String) {
        guard !controller.isRunning else {
            showError("Stop the search before importing results."); return
        }
        Task {
          do {
            let imported = try await Task.detached {
                try ResultsExport.decode(read())
            }.value
            guard !controller.isRunning else {
                showError("Stop the search before importing results."); return
            }
            query = imported.query
            controller.loadImported(seeds: imported.seeds, dropped: imported.dropped,
                                    query: imported.query, trinkets: imported.trinkets)
            var notice = "Imported \(imported.seeds.count) seed\(imported.seeds.count == 1 ? "" : "s") from \(source)"
            if imported.dropped > 0 {
                notice += " · \(imported.dropped) duplicate or over-limit entr\(imported.dropped == 1 ? "y" : "ies") dropped"
            }
            if let version = imported.shpdVersion, version != EngineInfo.shared.shpdVersion {
                notice += " · made for Shattered Pixel Dungeon v\(version); this app targets v\(EngineInfo.shared.shpdVersion), so seeds may generate differently"
            }
            importNotice = notice
            showResults = true
            tab = .finder
          } catch { showError(AndroidCopy.importError(error, source: source)) }
        }
    }

    func export() {
        guard let query = controller.exportQuery, !controller.results.isEmpty else {
            showError("Run a search first — there are no results to export yet."); return
        }
        let text = ResultsExport.encode(query, seeds: controller.results.map(\.seed),
            appVersion: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0",
            trinkets: controller.results.map(\.selectedTrinket))
        guard !text.isEmpty else { showError("This search could not be shared."); return }
        exportDocument = ResultsDocument(text: text)
    }

    func showError(_ message: String, title: String = "Results transfer") {
        alertTitle = title; alertMessage = message
    }
}

struct SeedSeekerRootView: View {
    @Bindable var model: AppModel
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        TabView(selection: $model.tab) {
            Tab("Finder", systemImage: "magnifyingglass", value: AppTab.finder) {
                NavigationStack {
                    FinderView(model: model)
                        .navigationTitle("Seed Seeker")
                        .navigationBarTitleDisplayMode(.inline)
                }
            }
            Tab("Scout", systemImage: "mappin.and.ellipse", value: AppTab.scout) {
                NavigationStack {
                    ScoutView(model: model.scout, query: model.query,
                              results: model.controller.results, onSelectResult: model.scoutResult)
                        .toolbar {
                            ToolbarItemGroup(placement: .topBarTrailing) {
                                Button("Settings", systemImage: "gearshape") { model.sheet = .settings }
                                Button("About and licenses", systemImage: "info.circle") { model.sheet = .about }
                            }
                        }
                }
            }
        }
        .sheet(item: $model.sheet) { destination in
            switch destination {
            case .settings: AppSettingsView()
            case .searchSettings:
                SearchSettingsView(query: $model.query, enabled: !model.controller.isRunning,
                                   challengesEnabled: !model.scout.loading)
            case .about: AboutView()
            case .presets: PresetsView(query: $model.query, presets: $model.presets)
            }
        }
        .sheet(item: $model.sharedLink) { payload in ShareSheet(items: [payload.text]) }
        .fileImporter(isPresented: $model.showingImporter, allowedContentTypes: [.json, .plainText]) { result in
            if case .success(let url) = result { model.importFile(url) }
        }
        .fileExporter(isPresented: Binding(get: { model.exportDocument != nil }, set: { if !$0 { model.exportDocument = nil } }),
            document: model.exportDocument, contentType: .json, defaultFilename: ResultsExport.suggestedFileName) { result in
            if case .failure(let error) = result { model.showError("Export failed: \(error.localizedDescription)") }
        }
        .alert(model.alertTitle, isPresented: Binding(get: { model.alertMessage != nil }, set: { if !$0 { model.alertMessage = nil } })) {
            Button("OK", role: .cancel) { model.alertMessage = nil }
        } message: { Text(model.alertMessage ?? "") }
        .onOpenURL(perform: model.open)
        .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
            if let url = activity.webpageURL { model.open(url) }
        }
        .task { model.backgroundSearch.didBecomeActive() }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background { model.backgroundSearch.didEnterBackground() }
            if phase == .active { model.backgroundSearch.didBecomeActive() }
        }
    }
}

struct ResultsDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }
    var text: String
    init(text: String) { self.text = text }
    init(configuration: ReadConfiguration) throws {
        text = String(decoding: configuration.file.regularFileContents ?? Data(), as: UTF8.self)
    }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: Data(text.utf8))
    }
}

struct SharePayload: Identifiable { let id = UUID(); let text: String }
struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]
    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }
    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
