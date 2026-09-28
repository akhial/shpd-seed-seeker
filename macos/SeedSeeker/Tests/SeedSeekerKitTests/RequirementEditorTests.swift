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
                             blanket: Bool = false, excludeResin: Bool = false) throws -> ItemRequirement {
        let item = try id.map { try XCTUnwrap(ItemCatalog.findById($0), "unknown catalog item \($0)") }
        return try ItemRequirement(key: key, item: item, upgrade: upgrade,
                                   kind: kind ?? item?.kind ?? .weapon, upgradeMatch: upgradeMatch,
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

        // A concrete ring stack: its copies fold behind the ×3 badge.
        let rings = board.items[0]
        XCTAssertEqual(rings.members, [1])
        XCTAssertEqual(rings.extras, [2, 3])
        XCTAssertEqual(rings.countBadge?.text, "×3")
        XCTAssertEqual(rings.countBadge?.tooltip, "3 of the same kind")
        XCTAssertNil(rings.totalBadge)
        XCTAssertEqual(rings.stack.count, 3)
        XCTAssertTrue(rings.stack.canCountLevels)
        XCTAssertEqual(rings.stack.levelCapacity, 11)
        XCTAssertEqual(rings.stack.countRange, 1...3)
        XCTAssertNil(board.item(holding: 2), "a hidden copy is no visible row")
        let might = try XCTUnwrap(rings.chips.first)
        XCTAssertEqual(might.catalogItem?.id, "ring_might")
        XCTAssertEqual(might.kind, .ring)
        XCTAssertEqual(might.tags.map(\.text), ["+2"])
        XCTAssertEqual(might.tags.map(\.isUpgrade), [true])
        XCTAssertEqual(might.relations.map(\.glyph), [.times])
        XCTAssertEqual(might.refusal(onto: 5)?.reason, "mixed_category_stack")

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
        XCTAssertEqual(cluster.anchor, 5)
        XCTAssertEqual(board.item(holding: 6)?.id, "c1")
        let excluded = try XCTUnwrap(board.chip(6))
        XCTAssertTrue(excluded.inCluster)
        XCTAssertTrue(excluded.canDetach)
        XCTAssertEqual(excluded.trailingTags.map(\.text), ["No resin"])
        XCTAssertEqual(excluded.relations.first?.glyph, .or)
        XCTAssertEqual(excluded.relations.first?.text, "Wand of Fireblast")

        // A trinket never stacks; a blanket lists its effects in catalog order.
        XCTAssertFalse(board.items[3].stack.canChangeCount)
        XCTAssertEqual(board.chip(7)?.tags.map(\.text), ["Transmute ≤3"])
        XCTAssertTrue(board.items[4].blanket)
        XCTAssertEqual(board.chip(8)?.effect?.effects, ["Viscosity", "Brimstone"])

        let resin = try XCTUnwrap(board.resin)
        XCTAssertEqual(resin.name, "Arcane Resin")
        XCTAssertEqual(resin.tags.map(\.text), ["Auto", "Mage +2"])
        XCTAssertEqual(resin.tooltip, "Heap")
        XCTAssertTrue(resin.uncursed)
        XCTAssertNotNil(resin.amountTooltip)
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
        XCTAssertEqual(refused.refusal?.reason, "mixed_category_stack")
        XCTAssertEqual(refused.refusal?.message, "Copies can only be grouped with the same item type.")

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
                     "board-stack-total", "board-copy-depth", "board-save-new", "board-join-trades-copies"] {
            let rows = try XCTUnwrap(try response(name)["rows"] as? [[String: Any]], name)
            XCTAssertFalse(rows.isEmpty, name)
            for row in rows {
                let requirement = try XCTUnwrap(ResultsExport.decodeRow(row), "\(name): \(row)")
                XCTAssertEqual(ResultsExport.encodeRow(requirement) as NSDictionary, row as NSDictionary, name)
            }
        }
    }

    // MARK: - Edits through the engine

    /// A drop joins, a join across a stack's category is refused and changes
    /// nothing, and the board already said so before the drop.
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

        let stacked = [try requirement(1, item: "ring_might", upgrade: 2, upgradeMatch: .exactly),
                       try requirement(2, item: "ring_might"),
                       try requirement(3, kind: .wand)]
        XCTAssertEqual(RequirementBoard.of(stacked).chip(3)?.refusal(onto: 1)?.reason, "mixed_category_stack")
        let refused = try XCTUnwrap(RequirementBoard.apply([.join(source: 3, target: 1)], to: stacked))
        XCTAssertFalse(refused.changed)
        XCTAssertEqual(refused.rows, stacked)
        XCTAssertEqual(refused.refusal?.message, "Copies can only be grouped with the same item type.")
    }

    /// Stack edits in one request, the way the cluster's stack sheet sends
    /// its count and copy floor together.
    func testStackEditsRunInOrder() throws {
        let cluster = [try requirement(1, item: "spear", alternativeGroup: 1),
                       try requirement(2, item: "mace", alternativeGroup: 1)]
        let board = RequirementBoard.of(cluster)
        XCTAssertTrue(try XCTUnwrap(board.item(holding: 1)).stack.canGrow)
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 3), .setCopyDepth(1, 9)], to: cluster))
        XCTAssertTrue(grown.changed)
        let item = try XCTUnwrap(grown.item(holding: 2))
        XCTAssertEqual(item.stack.count, 3)
        XCTAssertEqual(item.stack.copyDepth, 9)
        XCTAssertEqual(item.countBadge?.text, "×3")
        XCTAssertNoThrow(try SearchRequest(requirements: grown.rows))
        let shrunk = try XCTUnwrap(RequirementBoard.apply([.setCount(2, 1)], to: grown.rows))
        XCTAssertEqual(shrunk.item(holding: 1)?.stack.count, 1)
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
        let cluster = try XCTUnwrap(RequirementBoard.of(rows).item(holding: 1))
        XCTAssertTrue(cluster.stack.canGrow)
        let refused = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 2)], to: rows))
        XCTAssertFalse(refused.changed)
        XCTAssertEqual(refused.rows, rows)
        XCTAssertEqual(refused.refusal?.reason, "no_free_group")
        XCTAssertEqual(refused.refusal?.message,
                       "Every group label is in use. Remove a stack or a combined level first.")
    }

    /// A stack of wands kept out of Auto resin grows plain copies — only the
    /// anchor is excluded — so it folds into one chip and stays searchable.
    func testAnExcludedWandStackFoldsIntoOneChip() throws {
        let excluded = try requirement(1, kind: .wand, upgrade: 3, upgradeMatch: .exactly, excludeResin: true)
        let grown = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 2)], to: [excluded]))
        XCTAssertEqual(grown.items.count, 1)
        XCTAssertEqual(grown.items.first?.stack.count, 2)
        XCTAssertEqual(grown.rows.map(\.excludeResin), [true, false])
        XCTAssertTrue(grown.problems.isEmpty)
        XCTAssertNoThrow(try SearchRequest(requirements: grown.rows))

        let named = try requirement(1, item: "wand_frost", excludeResin: true)
        let three = try XCTUnwrap(RequirementBoard.apply([.setCount(1, 3)], to: [named]))
        XCTAssertEqual(three.items.map(\.stack.count), [3])
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
        XCTAssertEqual(chip.tooltip, "Locked chest")
        XCTAssertNil(chip.amountTooltip)
        XCTAssertNil(SavedQuery().board.resin)
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
        XCTAssertEqual(grown.item(holding: wandKey)?.stack.count, 2)
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
