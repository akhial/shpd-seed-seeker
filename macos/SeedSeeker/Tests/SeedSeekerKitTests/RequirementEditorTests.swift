import Foundation
import XCTest
@testable import SeedSeekerKit

/// The requirement editor's rules are the shared core's
/// (`docs/requirement-editor.md`); its own tests hold the board, the sheet
/// and the problem list. These hold the app's bridge to them: the golden
/// request/response pairs the core pins answer the same through the linked
/// engine, the typed views read them, rows survive the app's codec, and the
/// edits the boards send land — or are refused — as the core decides. The
/// sheet's bridge has its own cases in `RequirementSheetTests`.
final class RequirementEditorTests: XCTestCase {
    // MARK: - Fixtures

    private static let fixtureDirectory = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // RequirementEditorTests.swift -> SeedSeekerKitTests
        .deletingLastPathComponent() // -> Tests
        .deletingLastPathComponent() // -> SeedSeeker
        .deletingLastPathComponent() // -> macos
        .deletingLastPathComponent() // -> repository root
        .appendingPathComponent("crates/seedfinder-core/tests/fixtures/editor")

    private func fixture(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: Self.fixtureDirectory.appendingPathComponent("\(name).json"))
        return try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any], name)
    }

    private func response(_ name: String) throws -> [String: Any] {
        try XCTUnwrap(try fixture(name)["response"] as? [String: Any], name)
    }

    private func requirement(_ key: Int64, item id: String? = nil, kind: ItemKind? = nil,
                             upgrade: Int = 0, upgradeMatch: UpgradeMatch = .any,
                             maximumDepth: Int? = nil, alternativeGroup: Int? = nil,
                             identityGroup: Int? = nil,
                             blanket: Bool = false, excludeResin: Bool = false) throws -> ItemRequirement {
        let item = try id.map { try XCTUnwrap(ItemCatalog.findById($0), "unknown catalog item \($0)") }
        return try ItemRequirement(key: key, item: item, upgrade: upgrade,
                                   kind: kind ?? item?.kind ?? .weapon, upgradeMatch: upgradeMatch,
                                   identityGroup: identityGroup,
                                   maximumDepth: maximumDepth, alternativeGroup: alternativeGroup,
                                   blanket: blanket, excludeResin: excludeResin)
    }

    // MARK: - The golden pairs

    /// Every pair the core pins — both envelopes, the error documents
    /// included — answers the same through the engine the app links.
    func testGoldenFixturesAnswerTheSameThroughTheLinkedEngine() throws {
        let names = try FileManager.default.contentsOfDirectory(atPath: Self.fixtureDirectory.path)
            .filter { $0.hasSuffix(".json") }.sorted()
        XCTAssertGreaterThan(names.count, 20, "no editor fixtures at \(Self.fixtureDirectory.path)")
        for name in names {
            let document = try fixture(String(name.dropLast(".json".count)))
            let request = try XCTUnwrap(document["request"], name)
            // A request stored as a string is sent as that very text: the
            // fixtures for bytes that are not JSON at all.
            let bytes: Data
            if let text = request as? String {
                bytes = Data(text.utf8)
            } else {
                bytes = try JSONSerialization.data(withJSONObject: request)
            }
            let packet = document["envelope"] as? String == "requirement_board"
                ? EditorEngine.boardText(bytes) : EditorEngine.editorText(bytes)
            let answer = try JSONSerialization.jsonObject(with: try XCTUnwrap(packet, name))
            XCTAssertEqual(try XCTUnwrap(answer as? NSDictionary, name),
                           try XCTUnwrap(document["response"] as? NSDictionary, name), name)
        }
    }

    /// The tour — every kind of entry and the Auto resin chip — reads into
    /// the typed board the views draw.
    func testTheTourReadsIntoTheTypedBoard() throws {
        let board = try XCTUnwrap(RequirementBoard.decode(try response("board-tour"), sent: []))
        XCTAssertFalse(board.changed)
        XCTAssertNil(board.focus)
        XCTAssertNil(board.refusal)
        XCTAssertEqual(board.ordinaryCount, 4)
        XCTAssertEqual(board.blanketCount, 1)
        XCTAssertEqual(board.items.map(\.id), ["r1", "r4", "c1", "r7", "r8"])
        XCTAssertEqual(board.section(blanket: true).map(\.id), ["r8"])
        XCTAssertTrue(board.problems.isEmpty)

        // A concrete ring stack: its copies fold behind the chip's ×3 badge.
        let rings = board.items[0]
        XCTAssertEqual(rings.members, [1])
        XCTAssertEqual(rings.extras, [2, 3])
        XCTAssertNil(board.item(holding: 2), "a hidden copy is no visible row")
        let might = try XCTUnwrap(rings.chips.first)
        XCTAssertEqual(might.countBadge?.text, "×3")
        XCTAssertEqual(might.countBadge?.tooltip, "3 of the same kind")
        XCTAssertNil(might.totalBadge)
        XCTAssertEqual(might.copies, [2, 3])
        XCTAssertEqual(might.stack.count, 3)
        XCTAssertTrue(might.stack.canCountLevels)
        XCTAssertEqual(might.stack.levelCapacity, 11)
        XCTAssertEqual(might.stack.countMax, 3)
        XCTAssertEqual(might.stack.countRange, 1...3)
        XCTAssertEqual(might.catalogItem?.id, "ring_might")
        XCTAssertEqual(might.kind, .ring)
        XCTAssertEqual(might.tags.map(\.text), ["+2"])
        XCTAssertEqual(might.tags.map(\.isUpgrade), [true])
        XCTAssertEqual(might.tags.map(\.style), [.upgrade])
        XCTAssertEqual(might.tags.map(\.tooltip), [nil], "a chip's tags have no hover text of their own")
        XCTAssertEqual(might.relations.map(\.glyph), [.times])
        // Every copy keeps its own chip's kind, so the ring stack may join
        // the wands: #190's refusal is lifted.
        XCTAssertEqual(might.join, [4, 5, 6, 7])
        XCTAssertNil(might.refusal(onto: 5))

        // A narrowed wildcard with any enchantment: no one glow to pulse.
        let melee = try XCTUnwrap(board.chip(4))
        XCTAssertEqual(melee.name, "Any melee")
        XCTAssertEqual(melee.title, "Any Tier 3+ melee weapon")
        XCTAssertNil(melee.catalogItem)
        XCTAssertEqual(melee.kind, .meleeWeapon)
        XCTAssertEqual(melee.family, .weapon)
        XCTAssertEqual(melee.tags.map(\.text), ["T3+", "+2↑", "F≤9"])
        XCTAssertEqual(melee.effect?.anyEnchantment, true)
        XCTAssertEqual(melee.effect?.glowNames, [])
        XCTAssertTrue(melee.uncursed)
        XCTAssertEqual(melee.join, [5, 6, 7])
        XCTAssertEqual(melee.description,
                       "Any Tier 3+ melee weapon, +2 or higher, any enchantment, uncursed, floors 1–9")

        // An either/or cluster, its members detachable.
        let cluster = board.items[2]
        XCTAssertEqual(cluster.cluster, 1)
        XCTAssertEqual(cluster.label, "Any of 2")
        // Menus name each entry as the core does: a chip's name, a cluster's
        // members' names joined.
        XCTAssertEqual(board.items.map(\.name),
                       ["Ring of Might", "Any melee", "Wand of Fireblast or Any wand", "Rat Skull", "Any armor"])
        XCTAssertEqual(cluster.anchor, 5)
        XCTAssertTrue(cluster.chips.allSatisfy { $0.countBadge == nil && $0.stack.count == 1 })
        XCTAssertEqual(board.item(holding: 6)?.id, "c1")
        let excluded = try XCTUnwrap(board.chip(6))
        XCTAssertTrue(excluded.inCluster)
        XCTAssertTrue(excluded.canDetach)
        XCTAssertEqual(excluded.trailingTags.map(\.text), ["No resin"])
        XCTAssertEqual(excluded.relations.first?.glyph, .or)
        XCTAssertEqual(excluded.relations.first?.text, "Wand of Fireblast")

        // A trinket never stacks; a blanket lists its effects in catalog order.
        let skull = try XCTUnwrap(board.chip(7)).stack
        XCTAssertFalse(skull.canChangeCount)
        XCTAssertEqual(skull.countMax, 1)
        XCTAssertEqual(skull.countRange, 1...1)
        XCTAssertEqual(board.chip(7)?.tags.map(\.text), ["Transmute ≤3"])
        XCTAssertTrue(board.items[4].blanket)
        XCTAssertEqual(board.chip(8)?.effect?.effects, ["Viscosity", "Brimstone"])

        // The resin chip's tags are the resin it counts, each explaining
        // itself on hover.
        let resin = try XCTUnwrap(board.resin)
        XCTAssertEqual(resin.name, "Arcane Resin")
        XCTAssertEqual(resin.tags.map(\.text), ["Auto", "Mage +2"])
        XCTAssertEqual(resin.tags.map(\.style), [.credit, .credit])
        XCTAssertEqual(resin.tags.map(\.tooltip), [
            "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies",
            "Starting Magic Missile contributes 2 resin",
        ])
        XCTAssertEqual(resin.tooltip, "Heap")
        XCTAssertTrue(resin.uncursed)
    }

    /// A fixed amount's tag has no hover text; the donor floor is a plain
    /// filter beside the credit, and the source is the chip's own hover text.
    func testTheResinChipsCreditIsTintedApartFromItsFilter() throws {
        let board = try XCTUnwrap(RequirementBoard.decode(try response("board-resin-credit"), sent: []))
        let resin = try XCTUnwrap(board.resin)
        XCTAssertEqual(resin.tags.map(\.text), ["≥4", "Mage +2", "F≤9"])
        XCTAssertEqual(resin.tags.map(\.style), [.credit, .credit, .plain])
        XCTAssertEqual(resin.tags.map(\.isCredit), [true, true, false])
        XCTAssertEqual(resin.tags.map(\.tooltip), [nil, "Starting Magic Missile contributes 2 resin", nil])
        XCTAssertEqual(resin.tooltip, "Locked chest")
        XCTAssertEqual(resin.details.last, "floors 1–9")

        // A style this build does not know reads as plain rather than
        // dropping the tag.
        let unknown = try XCTUnwrap(ChipTag(json: ["text": "≥4", "style": "sparkle", "tooltip": NSNull()]))
        XCTAssertEqual(unknown.style, .plain)
        XCTAssertNil(unknown.tooltip)
        XCTAssertEqual(ChipTag(json: ["text": "+2"])?.style, .plain)
    }

    /// Problems name the rows they blame; refusals and key repairs read too.
    func testProblemsRefusalsAndRepairsRead() throws {
        let problems = try XCTUnwrap(RequirementBoard.decode(try response("board-problems"), sent: []))
        XCTAssertFalse(problems.problems.isEmpty)
        XCTAssertTrue(problems.problems.allSatisfy { !$0.message.isEmpty })
        XCTAssertTrue(problems.items.contains { $0.problem != nil })

        let blankets = try XCTUnwrap(RequirementBoard.decode(try response("board-problems-blankets-only"), sent: []))
        XCTAssertEqual(blankets.problems.last?.message, "Add at least one ordinary requirement.")
        XCTAssertEqual(blankets.problems.last?.scope, .list)
        XCTAssertEqual(blankets.problems.last?.keys, [])

        let refused = try XCTUnwrap(RequirementBoard.decode(try response("board-join-refused"), sent: []))
        XCTAssertEqual(refused.refusal?.reason, "no_free_group")
        XCTAssertEqual(refused.refusal?.message,
                       "Every group label is in use. Remove a stack or a combined level first.")
        XCTAssertEqual(refused.chip(11)?.refusal(onto: 9)?.reason, "no_free_group")

        // A repaired list is rows this build models, read whole.
        let repaired = try XCTUnwrap(RequirementBoard.decode(try response("board-key-repair"), sent: []))
        XCTAssertTrue(repaired.changed)
        XCTAssertEqual(repaired.rekeyed, [BoardKeyChange(from: 0, to: 5), BoardKeyChange(from: 4, to: 6)])
        XCTAssertEqual(repaired.key(following: 0), 5)
        XCTAssertEqual(repaired.key(following: 9), 9)
        XCTAssertEqual(repaired.rows.map(\.key), [5, 7, 4, 6])
        XCTAssertEqual(repaired.rows.map(\.alternativeGroup), [nil, nil, 1, 1])
    }

    /// Rows the core writes read into the app's model and write back
    /// exactly: the canonical requirement plus the key and either/or label.
    func testRowsRoundTripThroughTheAppCodec() throws {
        for name in ["board-tour", "board-stack-concrete", "board-stack-wildcard", "board-stack-cluster",
                     "board-stack-total", "board-copy-depth", "board-save-new", "board-join-leaves-copies",
                     "board-stack-member", "board-cluster-alike-stacks", "board-join-member-moves-one",
                     "board-join-across-categories", "board-detach-one-copy", "board-remove-one-member"] {
            let rows = try XCTUnwrap(try response(name)["rows"] as? [[String: Any]], name)
            XCTAssertFalse(rows.isEmpty, name)
            for row in rows {
                let requirement = try XCTUnwrap(ResultsExport.decodeRow(row), "\(name): \(row)")
                XCTAssertEqual(ResultsExport.encodeRow(requirement) as NSDictionary, row as NSDictionary, name)
            }
        }
    }

    // MARK: - Edits through the engine

    /// A drop joins — across categories too, a stack keeping its own kind of
    /// copies — and a refused join changes nothing, as the board already
    /// said before the drop.
    func testJoinsLandAndRefusalsChangeNothing() throws {
        let rows = [try requirement(1, item: "spear", upgrade: 2, upgradeMatch: .exactly),
                    try requirement(2, item: "mace"),
                    try requirement(3, kind: .wand)]
        let joined = try XCTUnwrap(RequirementBoard.apply([.join(source: 3, target: 1)], to: rows))
        XCTAssertTrue(joined.changed)
        XCTAssertNil(joined.refusal)
        XCTAssertEqual(joined.focus, 3)
        XCTAssertEqual(joined.rows.map(\.key), [1, 3, 2])
        XCTAssertNotNil(joined.rows[0].alternativeGroup)
        XCTAssertEqual(joined.rows[0].alternativeGroup, joined.rows[1].alternativeGroup)
        XCTAssertEqual(joined.ordinaryCount, 2)
        XCTAssertNoThrow(try SearchRequest(requirements: joined.rows))
        // The answer is also the board of the list it returns.
        XCTAssertEqual(RequirementBoard.of(joined.rows).items.map(\.id), joined.items.map(\.id))

        // #190 refused a wand dropped on a ring stack; now the ring keeps
        // its stack as a member: two Rings of Might, or the wand.
        let stacked = [try requirement(1, item: "ring_might", upgrade: 2, upgradeMatch: .exactly),
                       try requirement(2, item: "ring_might"),
                       try requirement(3, kind: .wand)]
        XCTAssertEqual(RequirementBoard.of(stacked).chip(3)?.join, [1])
        let across = try XCTUnwrap(RequirementBoard.apply([.join(source: 3, target: 1)], to: stacked))
        XCTAssertTrue(across.changed)
        XCTAssertNil(across.refusal)
        XCTAssertEqual(across.focus, 3)
        XCTAssertEqual(across.rows.map(\.key), [1, 3, 2])
        XCTAssertEqual(across.rows.map(\.identityGroup), [1, nil, 1])
        XCTAssertEqual(across.rows.map(\.alternativeGroup), [1, 1, nil])
        XCTAssertEqual(across.items.map(\.id), ["c1"])
        XCTAssertEqual(across.chip(1)?.countBadge?.text, "×2")
        XCTAssertEqual(across.chip(1)?.copies, [2])
        XCTAssertNil(across.chip(3)?.countBadge)
        XCTAssertNoThrow(try SearchRequest(requirements: across.rows))

        // With every group label in use, Frost ×2 has none to keep its stack
        // as a member: the join is refused, and the list is left as it was.
        var full: [ItemRequirement] = []
        for group in 1...4 {
            for copy in 0..<2 {
                full.append(try requirement(Int64(8 + 2 * group + copy), kind: .armor, identityGroup: group))
            }
        }
        full.append(contentsOf: [try requirement(1, item: "wand_frost"), try requirement(2, item: "wand_frost"),
                                 try requirement(3, item: "wand_disintegration")])
        XCTAssertEqual(RequirementBoard.of(full).chip(3)?.refusal(onto: 1)?.reason, "no_free_group")
        let refused = try XCTUnwrap(RequirementBoard.apply([.join(source: 3, target: 1)], to: full))
        XCTAssertFalse(refused.changed)
        XCTAssertEqual(refused.rows, full)
        XCTAssertEqual(refused.refusal?.message,
                       "Every group label is in use. Remove a stack or a combined level first.")
    }

    /// Stack edits in one request run in order, and a member's stack is its
    /// own: Spear grows to ×3 while Mace stays one item.
    func testStackEditsRunInOrder() throws {
        let cluster = [try requirement(1, item: "spear", alternativeGroup: 1),
                       try requirement(2, item: "mace", alternativeGroup: 1)]
        let board = RequirementBoard.of(cluster)
        XCTAssertTrue(try XCTUnwrap(board.chip(1)).stack.canGrow)
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 3), .setCopyDepth(1, 9)], to: cluster))
        XCTAssertTrue(grown.changed)
        XCTAssertEqual(grown.focus, 1)
        let spear = try XCTUnwrap(grown.chip(1))
        XCTAssertEqual(spear.stack.count, 3)
        XCTAssertEqual(spear.stack.copyDepth, 9)
        XCTAssertEqual(spear.countBadge?.text, "×3")
        XCTAssertEqual(spear.copies, [3, 4])
        XCTAssertFalse(spear.stack.canCountLevels, "a member counts no levels")
        let mace = try XCTUnwrap(grown.chip(2))
        XCTAssertEqual(mace.stack.count, 1)
        XCTAssertNil(mace.countBadge)
        XCTAssertEqual(grown.item(holding: 2)?.extras, [3, 4])
        XCTAssertNoThrow(try SearchRequest(requirements: grown.rows))
        XCTAssertEqual(RequirementBoard.apply([.setCount(2, 1)], to: grown.rows)?.changed, false)
        let shrunk = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 1)], to: grown.rows))
        XCTAssertEqual(shrunk.chip(1)?.stack.count, 1)
        XCTAssertEqual(shrunk.rows.map(\.key), [1, 2])
    }

    /// A chip's stack reads from the chip: a stack that cannot grow only
    /// sheds, its count stepper running down from its count as the core
    /// bounds it, and a chip whose answer has no stack is one item with
    /// nothing to step rather than no chip at all.
    func testAChipsStackReadsFromTheChip() throws {
        let answer = #"""
        {"key": 1, "name": "Rat Skull", "copies": [2],
         "badges": {"count": {"text": "×2", "compact_text": "×2", "tooltip": "2 of the same kind"}, "total": null},
         "stack": {"count": 2, "max": 3, "can_grow": false, "can_change_count": true, "count_max": 2,
                   "count_text": "×2"}}
        """#
        let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(answer.utf8)) as? [String: Any])
        let sheds = try XCTUnwrap(BoardChip(json: object))
        XCTAssertEqual(sheds.copies, [2])
        XCTAssertEqual(sheds.countBadge?.compactText, "×2")
        XCTAssertNil(sheds.totalBadge)
        XCTAssertFalse(sheds.stack.canGrow)
        XCTAssertTrue(sheds.stack.canChangeCount)
        XCTAssertEqual(sheds.stack.max, 3)
        XCTAssertEqual(sheds.stack.countRange, 1...2)

        let bare = try XCTUnwrap(BoardChip(json: ["key": NSNumber(value: 1), "name": "Spear"]))
        XCTAssertEqual(bare.stack.count, 1)
        XCTAssertFalse(bare.stack.canChangeCount)
        XCTAssertEqual(bare.stack.countRange, 1...1)
        XCTAssertNil(bare.countBadge)
        XCTAssertEqual(bare.copies, [])
    }

    /// Alike member stacks share one label and one copy: each member of
    /// {Frost ×2 | Disintegration ×2 | Lightning} shows its own ×2, and
    /// the cluster none.
    func testAlikeMemberStacksShowOnEveryMember() throws {
        let board = try XCTUnwrap(RequirementBoard.decode(try response("board-cluster-alike-stacks"), sent: []))
        let cluster = try XCTUnwrap(board.items.first)
        XCTAssertEqual(cluster.members, [1, 2, 3])
        XCTAssertEqual(cluster.extras, [4])
        XCTAssertEqual(cluster.chips.map { $0.countBadge?.text }, ["×2", "×2", nil])
        XCTAssertEqual(cluster.chips.map(\.copies), [[4], [4], []])
        XCTAssertEqual(cluster.chips.map(\.stack.count), [2, 2, 1])
        XCTAssertTrue(cluster.chips.allSatisfy { !$0.stack.canCountLevels && $0.totalBadge == nil })
    }

    /// A count the board offers can still be refused when every group label
    /// is in use; the menus and steppers show the message, and nothing changes.
    func testACountWithNoFreeGroupIsRefused() throws {
        var rows: [ItemRequirement] = []
        for group in 1...4 {
            for copy in 0..<2 {
                rows.append(try ItemRequirement(key: Int64(8 + 2 * group + copy), item: nil, upgrade: 0,
                                                kind: .armor, upgradeMatch: .any, identityGroup: group))
            }
        }
        rows.append(try requirement(1, item: "spear", alternativeGroup: 1))
        rows.append(try requirement(2, item: "mace", alternativeGroup: 1))
        XCTAssertTrue(try XCTUnwrap(RequirementBoard.of(rows).chip(1)).stack.canGrow)
        let refused = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 2)], to: rows))
        XCTAssertFalse(refused.changed)
        XCTAssertEqual(refused.rows, rows)
        XCTAssertEqual(refused.refusal?.reason, "no_free_group")
        XCTAssertEqual(refused.refusal?.message,
                       "Every group label is in use. Remove a stack or a combined level first.")
    }

    /// A member's iOS "How many" sheet: each control is one board edit, and
    /// the copies' floor control — words, stops and the floor it turns on
    /// at — is the core's, read off the member's own sheet, which shows it.
    func testAMembersCopyFloorComesFromTheCore() throws {
        let cluster = [try requirement(1, item: "spear", alternativeGroup: 1),
                       try requirement(2, item: "mace", alternativeGroup: 1)]
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 3)], to: cluster))
        let stack = try XCTUnwrap(grown.chip(1)).stack
        XCTAssertTrue(stack.canSetCopyDepth)
        XCTAssertEqual(stack.countText, "×3")
        XCTAssertNil(stack.copyDepth)

        let sheet = try XCTUnwrap(RequirementSheet.open(rows: grown.rows, key: 1))
        XCTAssertTrue(sheet.form.inCluster)
        XCTAssertTrue(sheet.form.stack.visible)
        XCTAssertEqual(sheet.form.stack.count, 3)
        XCTAssertFalse(sheet.form.stack.countLevels.visible)
        let off = sheet.form.stack.copyDepth
        XCTAssertFalse(off.enabled)
        XCTAssertEqual(off.value, 4)
        XCTAssertFalse(off.options.contains(5))

        let limited = try XCTUnwrap(RequirementBoard.apply([.setCopyDepth(1, off.value)], to: grown.rows))
        XCTAssertTrue(limited.changed)
        XCTAssertEqual(limited.chip(1)?.stack.copyDepth, 4)
        XCTAssertNil(limited.chip(2)?.stack.copyDepth)
        let on = try XCTUnwrap(RequirementSheet.open(rows: limited.rows, key: 1)).form.stack.copyDepth
        XCTAssertTrue(on.enabled)
        XCTAssertEqual(on.label, "Limit the extra copies to a floor")
        XCTAssertEqual(on.valueLabel, "Copies within first 4 floors")
        XCTAssertNoThrow(try SearchRequest(requirements: limited.rows))

        let cleared = try XCTUnwrap(RequirementBoard.apply([.setCopyDepth(1, nil)], to: limited.rows))
        XCTAssertNil(cleared.chip(1)?.stack.copyDepth)
    }

    /// A stack of wands kept out of Auto resin grows plain copies — only the
    /// anchor is excluded — so it folds into one chip and stays searchable.
    func testAnExcludedWandStackFoldsIntoOneChip() throws {
        let excluded = try requirement(1, kind: .wand, upgrade: 3, upgradeMatch: .exactly, excludeResin: true)
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 2)], to: [excluded]))
        XCTAssertEqual(grown.items.count, 1)
        XCTAssertEqual(grown.items.first?.chips.first?.stack.count, 2)
        XCTAssertEqual(grown.rows.map(\.excludeResin), [true, false])
        XCTAssertTrue(grown.problems.isEmpty)
        XCTAssertNoThrow(try SearchRequest(requirements: grown.rows))

        let named = try requirement(1, item: "wand_frost", excludeResin: true)
        let three = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 3)], to: [named]))
        XCTAssertEqual(three.items.flatMap(\.chips).map(\.stack.count), [3])
        // Its sheet opens on the whole stack and saves it back as it was.
        let sheet = try XCTUnwrap(RequirementSheet.open(rows: three.rows, key: 1))
        XCTAssertEqual(sheet.form.stack.count, 3)
        guard case .saved(let resaved) = try XCTUnwrap(sheet.save(onto: three.rows)) else {
            return XCTFail("an unchanged stack must save")
        }
        XCTAssertFalse(resaved.changed)
        XCTAssertEqual(resaved.rows, three.rows)
    }

    /// The resin condition the query holds draws its chip; a query without
    /// one draws none.
    func testTheResinChipFollowsTheQuery() throws {
        XCTAssertNil(BoardResin(amount: 0, auto: false, filter: .init()))
        let query = SavedQuery(arcaneResin: 4, arcaneResinFilter: ArcaneResinFilter(maximumDepth: 9, source: .lockedChest))
        let chip = try XCTUnwrap(query.board.resin)
        XCTAssertEqual(chip.tags.map(\.text), ["≥4", "F≤9"])
        XCTAssertEqual(chip.tags.map(\.style), [.credit, .plain])
        XCTAssertNil(chip.tags.first?.tooltip, "a fixed amount needs no explaining")
        XCTAssertEqual(chip.tooltip, "Locked chest")
        XCTAssertNil(SavedQuery().board.resin)
        let auto = try XCTUnwrap(SavedQuery(arcaneResinAuto: true).board.resin)
        XCTAssertEqual(auto.tags.first?.text, "Auto")
        XCTAssertEqual(auto.tags.first?.tooltip,
                       "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies")
    }

    // MARK: - Keys

    /// A loaded list reaches the board keyed 1…n and in the canonical
    /// encoding — here with its wide either/or labels compacted — so the
    /// board has nothing left to repair.
    func testLoadedListsReachTheBoardCanonical() throws {
        let rows = [try requirement(0, kind: .wand),
                    try requirement(Int64.max, kind: .ring, alternativeGroup: 300),
                    try requirement(Int64.max, kind: .armor, alternativeGroup: 300)]
        let loaded = rows.loadedForBoard()
        XCTAssertEqual(loaded.map(\.key), [1, 2, 3])
        XCTAssertEqual(loaded.map(\.alternativeGroup), [nil, 1, 1])
        XCTAssertEqual(RequirementBoard.of(loaded).changed, false)
        let query = SavedQuery(requirements: rows).loadedForBoard()
        XCTAssertEqual(query.requirements, loaded)
    }

    /// A board handed keys it must repair answers with the repaired keys,
    /// and an edit naming them lands on the same rows.
    func testRepairedKeysNameTheSameRows() throws {
        let rows = [try requirement(0, kind: .wand), try requirement(4, kind: .ring),
                    try requirement(4, kind: .armor)]
        let board = RequirementBoard.of(rows)
        XCTAssertTrue(board.changed)
        XCTAssertEqual(board.rekeyed.count, 2)
        let wandKey = board.key(following: 0)
        XCTAssertNotEqual(wandKey, 0)
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(wandKey, 2)], to: rows))
        XCTAssertEqual(grown.chip(wandKey)?.stack.count, 2)
        XCTAssertFalse(grown.rows.contains { $0.key == 0 })
    }

    /// A request the core cannot read answers an error document, which the
    /// bridge reads as no answer rather than an empty board.
    func testErrorDocumentsAreNoAnswer() throws {
        XCTAssertNil(EditorEngine.board(["rows": [["kind": "wand"]]]))
        let packet = try XCTUnwrap(EditorEngine.boardText(Data("{\"rows\": [".utf8)))
        let answer = try XCTUnwrap(try JSONSerialization.jsonObject(with: packet) as? [String: Any])
        XCTAssertNotNil(answer["error"] as? String)
    }
}
