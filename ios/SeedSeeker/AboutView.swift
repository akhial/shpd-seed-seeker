// SPDX-License-Identifier: GPL-3.0-or-later
import SeedSeekerKit
import SwiftUI

struct AboutView: View {
    @Environment(\.dismiss) private var dismiss
    @State private var showingLicense = false
    @State private var licenseText = ""

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    HStack(spacing: 16) {
                        Image("AppIconPreview")
                            .resizable().scaledToFit().frame(width: 68, height: 68)
                            .clipShape(.rect(cornerRadius: 18))
                            .accessibilityHidden(true)
                        Link("Seed Seeker", destination: URL(string: "https://github.com/akhial/shpd-seed-seeker")!)
                            .font(.title2.weight(.semibold)).foregroundStyle(.primary)
                    }
                    .padding(.vertical, 6)
                    Text("An extremely fast seed finder for Shattered Pixel Dungeon, written in Rust — with native apps for Android, Linux, macOS, and Windows.")
                        .foregroundStyle(.secondary)
                }

                Section("Acknowledgements") {
                    Text("Seed Seeker reimplements the generation of Shattered Pixel Dungeon by Evan Debenham, itself based on Pixel Dungeon by Oleg Dolya.")
                    Text("Elektrochecker's shpd-seed-finder serves as an oracle for this project's parity tests.")
                }

                Section("License and identity") {
                    Text("This project is GPL-3.0-or-later. It contains a derived generation implementation and an unchanged item sprite atlas from Shattered Pixel Dungeon.")
                    attribution("Pixel Dungeon", value: "© 2012–2015 Oleg Dolya / Watabou",
                                url: "https://github.com/watabou/pixel-dungeon")
                    attribution("Shattered Pixel Dungeon", value: "© 2014–2026 Evan Debenham",
                                url: "https://github.com/00-Evan/shattered-pixel-dungeon")
                    attribution("Upstream", value: "Shattered Pixel Dungeon v\(EngineInfo.shared.shpdVersion)")
                    attribution("Release JAR SHA-256", value: EngineInfo.shared.shpdCommit)
                    attribution("Atlas SHA-256", value: "4774791518f960a4…7e8e7b5706")
                    attribution("Icon SHA-256", value: "38df728d32842d9f…24d7eb9b72")
                    Button(showingLicense ? "Hide full license" : "Read full license") {
                        showingLicense.toggle()
                    }
                    if showingLicense {
                        Text(licenseText)
                            .font(.system(.caption, design: .monospaced))
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }

                Section {
                    Text("Seed Seeker \(appVersion) · Shattered Pixel Dungeon v\(EngineInfo.shared.shpdVersion) profile")
                        .font(.caption).foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity).multilineTextAlignment(.center)
                }
                .listRowBackground(Color.clear)
            }
            .navigationTitle("About & licenses")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .task { licenseText = loadLicense() }
        }
    }

    private var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
    }

    private func attribution(_ label: String, value: String, url: String? = nil) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            if let url, let destination = URL(string: url) {
                Link(label, destination: destination).font(.caption)
            } else {
                Text(label).font(.caption).foregroundStyle(.tint)
            }
            Text(value).font(.subheadline).foregroundStyle(.secondary).textSelection(.enabled)
        }
        .padding(.vertical, 3)
    }

    private func loadLicense() -> String {
        do {
            guard let url = Bundle.main.url(forResource: "LICENSE-ShatteredPixelDungeon", withExtension: "txt") else {
                throw CocoaError(.fileNoSuchFile)
            }
            return try String(contentsOf: url, encoding: .utf8)
        } catch {
            return "License text could not be loaded: \(error.localizedDescription)"
        }
    }
}
