import CSeedFinder
import Foundation
import XCTest
@testable import SeedSeekerKit

final class LevelMapTests: XCTestCase, @unchecked Sendable {
    private func request(_ depth: Int = 1, branch: Int = 0, challenges: Int = 0, trinket: String? = nil) -> LevelMapRequest {
        LevelMapRequest(seed: "AAA-AAA-AAA", depth: depth, branch: branch, challenges: challenges, selectedTrinket: trinket)
    }

    func testItemInspectionDecodesGeneratedUpgradesEnchantmentsAndCurses() async throws {
        let bundle = try await LevelMapClient.shared.load(request(7))
        let items = try XCTUnwrap(bundle.document.itemTooltips).flatMap(\.items)
        let enchanted = try XCTUnwrap(items.first { $0.name == "Vorpal Assassin's Blade" })
        XCTAssertEqual(enchanted.upgrade, 1); XCTAssertEqual(enchanted.enchantment, "Vorpal")
        XCTAssertEqual(enchanted.cursed, false); XCTAssertNil(enchanted.curse)
        XCTAssertEqual(enchanted.glow?.color, [170, 102, 102]); XCTAssertEqual(enchanted.glow?.periodMs, 1000)
        let cursed = try XCTUnwrap(items.first { $0.curse == "Wondrous" })
        XCTAssertEqual(cursed.upgrade, 1); XCTAssertEqual(cursed.cursed, true); XCTAssertNil(cursed.enchantment)
        XCTAssertEqual(cursed.glow?.color, [0, 0, 0])
        XCTAssertTrue(items.contains { $0.upgrade == nil && $0.glow == nil })
    }

    func testRequestUsesEngineChallengeNamesAndExplicitNone() throws {
        let document = try XCTUnwrap(JSONSerialization.jsonObject(with: request(challenges: 104).encoded()) as? [String: Any])
        XCTAssertEqual(document["trinket"] as? String, "none")
        XCTAssertEqual(document["challenges"] as? [String], ["barren_land", "forbidden_runes", "into_darkness"])
        XCTAssertEqual(document["branch"] as? Int, 0)
        XCTAssertNil(document["query"])
        XCTAssertTrue(EngineInfo.shared.levelMapDepths.contains(5))
        XCTAssertTrue(EngineInfo.shared.levelMapDepths.contains(15))
        XCTAssertFalse(EngineInfo.shared.levelMapDepths.contains(10))
        XCTAssertNotEqual(request(), request(branch: 1))
        XCTAssertNotEqual(request(), request(trinket: "mimic_tooth"))
    }

    func testNativeScenesLoadBossesBranchesAssetsAndResolvedTrinket() async throws {
        let first = try await LevelMapClient.shared.load(request())
        XCTAssertEqual(first.document.schemaVersion, 3)
        XCTAssertNil(first.document.selectedTrinket)
        XCTAssertFalse(first.assets.isEmpty)
        for asset in first.document.assets {
            XCTAssertEqual(first.assets[asset.id]?.prefix(8), Data([137, 80, 78, 71, 13, 10, 26, 10]))
        }
        for depth in [5, 15] {
            let boss = try await LevelMapClient.shared.load(request(depth))
            XCTAssertEqual(boss.document.depth, depth)
            XCTAssertEqual(boss.document.kind, "regular")
        }
        var foundBranch = false
        for depth in 12...14 {
            let parent = try await LevelMapClient.shared.load(request(depth))
            guard let entry = parent.document.branches.first else { continue }
            let mine = try await LevelMapClient.shared.load(request(depth, branch: entry.branch))
            XCTAssertEqual(mine.document.kind, entry.kind)
            XCTAssertEqual(mine.document.branch, 1)
            XCTAssertTrue(mine.document.branches.isEmpty)
            foundBranch = true
            break
        }
        XCTAssertTrue(foundBranch)
        let world = try await ProductionSeedFinderEngine().scoutSeed("AAA-AAA-AAA", challenges: 0)
        let offer = try XCTUnwrap(world.trinketOrder.first?.id)
        let selected = try await LevelMapClient.shared.load(request(trinket: offer))
        XCTAssertEqual(selected.document.selectedTrinket, offer)
        let impDepth = try XCTUnwrap(world.quests.first(where: { $0.kind == .imp })?.depth)
        let vault = try await LevelMapClient.shared.load(request(impDepth, branch: 1))
        XCTAssertEqual(vault.document.kind, "imp_vault")
        XCTAssertFalse(vault.document.scene.emitters.isEmpty)
    }

    func testUnsupportedLocationAndSchemaFailWithoutPoisoningCache() async throws {
        do { _ = try await LevelMapClient.shared.load(request(10)); XCTFail("Unsupported floor must fail") }
        catch { XCTAssertTrue(error is SeedFinderEngineError) }
        let input = try request().encoded()
        let packet = try enginePacket { out, length in
            input.withUnsafeBytes { bytes in
                seedfinder_level_map(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count, out, length)
            }
        }
        var document = try XCTUnwrap(JSONSerialization.jsonObject(with: packet) as? [String: Any])
        document["schemaVersion"] = 99
        XCTAssertThrowsError(try LevelMapDocument.decode(JSONSerialization.data(withJSONObject: document)))
        document["schemaVersion"] = 3; document["width"] = Int.max
        XCTAssertThrowsError(try LevelMapDocument.decode(JSONSerialization.data(withJSONObject: document)))
        _ = try await LevelMapClient.shared.load(request())
    }

    private func emitter(start: Double?) throws -> LevelMapDocument.Emitter {
        var json: [String: Any] = [
            "cell": 0, "loopMs": 2000, "blend": "add", "wallMask": true,
            "image": ["kind": "fill", "rgba": [255, 255, 255, 255], "destination": [0, 0, 2, 2]],
            "velocity": [3, -2], "acceleration": [0, 4], "angularSpeed": 10,
            "alpha": ["points": [[0, 1000], [1000, 0]], "sqrt": false],
            "scale": ["points": [[0, 1000], [1000, 1000]], "sqrt": false],
            "scaleX": ["points": [[0, 0], [1000, 4000]], "sqrt": false],
            "scaleY": ["points": [[0, 1000], [1000, 0]], "sqrt": true],
            "particles": [["birthMs": 500, "lifespanMs": 1000, "position": [1000, 2000], "scale": 2000, "angle": 20]],
        ]
        if let start { json["startMs"] = start }
        return try JSONDecoder().decode(LevelMapDocument.Emitter.self, from: JSONSerialization.data(withJSONObject: json))
    }

    func testScheduledParticlesDoNotWrapIntoThePastAndFollowCurves() throws {
        let emitter = try emitter(start: 1000), particle = emitter.particles[0]
        XCTAssertNil(emitter.state(of: particle, at: 1499))
        let state = try XCTUnwrap(emitter.state(of: particle, at: 1750))
        XCTAssertEqual(state.x, 1.75, accuracy: 0.00001)
        XCTAssertEqual(state.y, 1.625, accuracy: 0.00001)
        XCTAssertEqual(state.scale, 2)
        XCTAssertEqual(state.scaleX, 1)
        XCTAssertEqual(state.alpha, 0.75)
        XCTAssertEqual(state.scaleY, sqrt(0.75), accuracy: 0.00001)
        XCTAssertEqual(state.angle, 22.5 * .pi / 180, accuracy: 0.00001)
        XCTAssertNil(emitter.state(of: particle, at: 2500))
        XCTAssertEqual(emitter.state(of: particle, at: 3750)?.x, state.x)
        let ambient = try self.emitter(start: nil)
        XCTAssertEqual(ambient.state(of: ambient.particles[0], at: 750)?.x, state.x)
    }

    func testGlowUsesContinuousTimeAndPreservesTriangularPeriod() throws {
        let glow = try JSONDecoder().decode(LevelMapDocument.Glow.self, from: Data(#"{"color":[255,0,0],"periodMs":1000}"#.utf8))
        XCTAssertEqual(glow.strength(at: 0), 0)
        XCTAssertEqual(glow.strength(at: 250), 0.15, accuracy: 0.00001)
        XCTAssertEqual(glow.strength(at: 1000), 0.6)
        XCTAssertEqual(glow.strength(at: 1750), 0.15, accuracy: 0.00001)
        XCTAssertEqual(glow.strength(at: 2000), 0)
    }

    func testReducedMotionResumesAtTheCapturedFirstTurn() throws {
        let uptime = 9 * 24 * 60 * 60 * 1000.0
        var clock = LevelMapClock(now: uptime)
        XCTAssertEqual(clock.elapsed(at: uptime, reducedMotion: true), 0)
        XCTAssertEqual(clock.elapsed(at: uptime + 5000, reducedMotion: true), 0)
        let first = clock.elapsed(at: uptime + 6000, reducedMotion: false)
        XCTAssertEqual(first, 0)
        XCTAssertEqual(clock.elapsed(at: uptime + 6250, reducedMotion: false), 250)
        let hazard = try emitter(start: 1000)
        XCTAssertNil(hazard.state(of: hazard.particles[0], at: first))
        XCTAssertNotNil(hazard.state(of: hazard.particles[0], at: clock.elapsed(at: uptime + 7500, reducedMotion: false)))
    }

    func testOnlyConflictingChoiceOptionsDim() throws {
        let item = try XCTUnwrap(ItemCatalog.findById("dagger"))
        let items = [
            ScoutItem(item: item, depth: 1, upgrade: 0, source: .chest, accessibility: .choice(group: 2, option: 0)),
            ScoutItem(item: item, depth: 1, upgrade: 0, source: .chest, accessibility: .choice(group: 2, option: 0)),
            ScoutItem(item: item, depth: 1, upgrade: 0, source: .chest, accessibility: .choice(group: 2, option: 1)),
        ]
        let status = ScoutChoiceStatus(items: items, matched: [0])
        XCTAssertFalse(status.isDimmed(items[0].accessibility, matched: true))
        XCTAssertFalse(status.isDimmed(items[1].accessibility, matched: false))
        XCTAssertTrue(status.isDimmed(items[2].accessibility, matched: false))
        XCTAssertFalse(status.isDimmed(.choice(group: 3, option: 1), matched: false))
        XCTAssertFalse(status.isDimmed(.independent, matched: false))
        XCTAssertEqual(ScoutChoiceStatus.letter(2), "C")
    }
}
