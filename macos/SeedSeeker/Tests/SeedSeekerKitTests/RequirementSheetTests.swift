import Foundation
import XCTest
@testable import SeedSeekerKit

/// The requirement sheet's bridge to the shared core
/// (`docs/requirement-editor.md`, "The sheet"): the golden sheets read into
/// the typed form both dialogs draw, every change the dialogs send is one the
/// core reads, and saves land — or are refused — as the core decides, the
/// Arcane Resin conversions included. The sheet's rules themselves are the
/// core's own tests'.
final class RequirementSheetTests: XCTestCase {
    // MARK: - Fixtures

    private static let fixtureDirectory = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // RequirementSheetTests.swift -> SeedSeekerKitTests
        .deletingLastPathComponent() // -> Tests
        .deletingLastPathComponent() // -> SeedSeeker
        .deletingLastPathComponent() // -> macos
        .deletingLastPathComponent() // -> repository root
        .appendingPathComponent("crates/seedfinder-core/tests/fixtures/editor")

    private func response(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: Self.fixtureDirectory.appendingPathComponent("\(name).json"))
        let document = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any], name)
        return try XCTUnwrap(document["response"] as? [String: Any], name)
    }

    private func fixtureForm(_ name: String) throws -> SheetForm {
        try XCTUnwrap(RequirementSheet(json: try response(name)), name).form
    }

    private func requirement(_ key: Int64, item id: String? = nil, kind: ItemKind? = nil,
                             upgrade: Int = 0, upgradeMatch: UpgradeMatch = .any,
                             maximumDepth: Int? = nil, alternativeGroup: Int? = nil,
                             identityGroup: Int? = nil, excludeResin: Bool = false) throws -> ItemRequirement {
        let item = try id.map { try XCTUnwrap(ItemCatalog.findById($0), "unknown catalog item \($0)") }
        return try ItemRequirement(key: key, item: item, upgrade: upgrade,
                                   kind: kind ?? item?.kind ?? .weapon, upgradeMatch: upgradeMatch,
                                   identityGroup: identityGroup, maximumDepth: maximumDepth,
                                   alternativeGroup: alternativeGroup, excludeResin: excludeResin)
    }

    /// The sheet after the user moved each control in turn.
    private func moved(_ sheet: RequirementSheet, _ changes: [SheetChange]) throws -> RequirementSheet {
        var current = sheet
        for change in changes {
            current = try XCTUnwrap(current.changing(change), "the core did not read \(change)")
        }
        return current
    }

    private func landed(_ outcome: SheetSaveOutcome?) throws -> SheetSaved {
        var result: SheetSaved?
        if case .saved(let saved)? = outcome { result = saved }
        return try XCTUnwrap(result, "the save was refused or not answered")
    }

    private func turnedBack(_ outcome: SheetSaveOutcome?) throws -> RequirementSheet {
        var result: RequirementSheet?
        if case .refused(let sheet)? = outcome { result = sheet }
        return try XCTUnwrap(result, "the save was not refused")
    }

    // MARK: - The golden sheets

    /// A chip's sheet: its stack from the board, the controls the core
    /// shows and hides, their bounds and their words.
    func testAChipsSheetReadsIntoTheTypedForm() throws {
        let sheet = try XCTUnwrap(RequirementSheet(json: try response("editor-open-row")))
        XCTAssertFalse(sheet.draft.isEmpty)
        let form = sheet.form
        XCTAssertEqual(form.mode, .edit)
        XCTAssertEqual(form.origin, .row(1))
        XCTAssertFalse(form.blanket)
        XCTAssertFalse(form.inCluster)
        XCTAssertFalse(form.resinPicked)
        XCTAssertEqual(form.title, "Ring of Might")
        XCTAssertEqual(form.preview?.title, "Ring of Might")
        XCTAssertEqual(form.preview?.key, 0)
        XCTAssertEqual(form.preview?.relations.first?.glyph, .sum)
        // The preview is the chip a save would draw, badges and stack
        // included, with no copy keys of its own.
        XCTAssertEqual(form.preview?.countBadge?.text, "≤2")
        XCTAssertEqual(form.preview?.totalBadge?.text, "Σ ≥ 3")
        XCTAssertEqual(form.preview?.totalBadge?.compactText, "Σ≥3")
        XCTAssertEqual(form.preview?.stack.total, 3)
        XCTAssertEqual(form.preview?.copies, [])
        XCTAssertEqual(form.category.value, "ring")
        XCTAssertEqual(form.category.options.map(\.label), ["Weapon", "Armor", "Wand", "Ring", "Trinket", "Artifact"])
        XCTAssertEqual(form.item.value, "ring_might")
        XCTAssertEqual(form.item.selected?.label, "Ring of Might")
        XCTAssertEqual(form.item.options.first?.label, "Any ring")
        XCTAssertNil(form.item.options.first?.value)
        XCTAssertEqual(form.item.options.first?.isCatalogItem, false)
        XCTAssertFalse(form.weaponType.visible)
        XCTAssertFalse(form.tier.visible)
        // A stack counting its levels speaks for their upgrades.
        XCTAssertFalse(form.upgrade.visible)
        XCTAssertTrue(form.stack.visible)
        XCTAssertEqual(form.stack.label, "Total item count")
        XCTAssertEqual(form.stack.count, 2)
        XCTAssertEqual(form.stack.range, 1...3)
        XCTAssertEqual(form.stack.valueLabel, "×2")
        XCTAssertFalse(form.stack.copyDepth.visible)
        let levels = form.stack.countLevels
        XCTAssertTrue(levels.visible)
        XCTAssertTrue(levels.enabled)
        XCTAssertEqual(levels.value, 3)
        XCTAssertEqual(levels.range, 1...8)
        XCTAssertTrue(levels.isAdjustable)
        XCTAssertEqual(levels.valueLabel, "≥ 3 across up to 2")
        XCTAssertEqual(levels.label, "Count levels together")
        XCTAssertEqual(levels.caption, "Each item counts its upgrade plus one, and spare items may go unused.")
        XCTAssertTrue(levels.captionVisible)
        // Floor sliders stop at the floors the core offers, past the empty
        // boss floors.
        XCTAssertTrue(form.floorLimit.visible)
        XCTAssertFalse(form.floorLimit.enabled)
        XCTAssertEqual(form.floorLimit.value, 4)
        XCTAssertEqual(form.floorLimit.label, "Limit this item to a floor")
        XCTAssertFalse(form.floorLimit.options.contains(5))
        XCTAssertEqual(form.floorLimit.options.first, 1)
        XCTAssertEqual(form.floorLimit.options.last, 24)
        XCTAssertEqual(form.floorLimit.index, 3)
        XCTAssertEqual(form.floorLimit.floor(at: 4), 6)
        XCTAssertEqual(form.floorLimit.floor(at: 99), 24)
        XCTAssertNil(form.source.value)
        XCTAssertEqual(form.source.options.first?.label, "Any")
        XCTAssertEqual(form.source.options.first(where: { $0.value == "locked_chest" })?.label, "Locked chest")
        XCTAssertEqual(form.uncursed.label, "Require uncursed")
        XCTAssertNil(form.uncursed.caption)
        XCTAssertFalse(form.resin.visible)
        XCTAssertTrue(form.errors.isEmpty)
        XCTAssertTrue(form.canSave)
    }

    /// A cluster member's sheet shows its own stack — its count and copy
    /// floor, never a combined level — and saves it: the Frost of
    /// {Frost ×2 | Disintegration} saved at ×3 grows Frost alone.
    func testAMembersSheetEditsItsOwnStack() throws {
        let form = try fixtureForm("editor-open-member")
        XCTAssertTrue(form.inCluster)
        XCTAssertTrue(form.stack.visible)
        XCTAssertEqual(form.stack.count, 2)
        XCTAssertEqual(form.stack.valueLabel, "×2")
        XCTAssertTrue(form.stack.copyDepth.visible)
        XCTAssertFalse(form.stack.countLevels.visible)
        XCTAssertEqual(form.preview?.countBadge?.text, "×2")
        XCTAssertFalse(try XCTUnwrap(form.preview).stack.canCountLevels)

        let rows = [try requirement(1, item: "wand_frost", alternativeGroup: 1, identityGroup: 1),
                    try requirement(2, item: "wand_disintegration", alternativeGroup: 1),
                    try requirement(3, kind: .wand, identityGroup: 1)]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, key: 1))
        XCTAssertEqual(opened.form.stack.count, 2)
        let grown = try moved(opened, [.count(3)])
        XCTAssertEqual(grown.form.stack.valueLabel, "×3")
        let saved = try landed(grown.save(onto: rows))
        XCTAssertTrue(saved.changed)
        XCTAssertEqual(saved.focus, 1)
        XCTAssertEqual(saved.rows.map(\.key), [1, 2, 3, 4])
        XCTAssertEqual(saved.rows.map(\.identityGroup), [1, nil, 1, 1])
        let board = RequirementBoard.of(saved.rows)
        XCTAssertEqual(board.chip(1)?.countBadge?.text, "×3")
        XCTAssertNil(board.chip(2)?.countBadge)
        XCTAssertEqual(try XCTUnwrap(RequirementSheet.open(rows: rows, key: 2)).form.stack.valueLabel, "×1")
        XCTAssertNoThrow(try SearchRequest(requirements: saved.rows))
    }

    /// A new chip starts on any weapon, its items under their tiers.
    func testANewSheetListsWeaponsUnderTheirTiers() throws {
        let form = try fixtureForm("editor-open-new")
        XCTAssertEqual(form.mode, .new)
        XCTAssertEqual(form.origin, .new)
        XCTAssertEqual(form.title, "Any weapon")
        XCTAssertEqual(form.preview?.title, "Any weapon")
        XCTAssertNil(form.item.value)
        let sections = form.item.sections
        XCTAssertNil(sections.first?.title)
        XCTAssertEqual(sections.first?.options.map(\.label), ["Any weapon"])
        XCTAssertEqual(sections.dropFirst().map(\.title), ["Tier 2", "Tier 3", "Tier 4", "Tier 5"])
        XCTAssertEqual(sections.map(\.options.count).reduce(0, +), form.item.options.count)
        // Arcane Resin is a wand-sheet choice.
        XCTAssertFalse(form.item.options.contains(where: \.isArcaneResin))
        for option in form.item.options where option.isCatalogItem {
            XCTAssertNotNil(ItemCatalog.findById(option.value ?? ""), option.label)
        }
        XCTAssertTrue(form.weaponType.visible)
        XCTAssertEqual(form.weaponType.options.map(\.value), ["any", "melee", "thrown"])
        XCTAssertTrue(form.upgrade.visible)
        XCTAssertEqual(form.upgrade.mode, "any")
        XCTAssertFalse(form.upgrade.valueVisible)
        XCTAssertEqual(form.upgrade.modes.map(\.label), ["Any", "Exactly", "At least"])
        XCTAssertTrue(form.effect.visible)
        XCTAssertEqual(form.effect.label, "Enchantment")
        XCTAssertFalse(form.effect.choicesVisible)
        XCTAssertEqual(form.effect.modes.map(\.label), ["Any", "Any enchantment", "Specific…"])
        XCTAssertTrue(form.stack.visible)
        XCTAssertEqual(form.stack.valueLabel, "×1")
    }

    /// The effect grid, a copy floor and a refused save read as drawn.
    func testEffectsCopyFloorsAndErrorsRead() throws {
        let effect = try fixtureForm("editor-change-effect").effect
        XCTAssertTrue(effect.choicesVisible)
        XCTAssertEqual(effect.groups.map(\.label), ["Enchantments", "Curses"])
        XCTAssertEqual(effect.choices(in: "enchantment").first?.value, "Blazing")
        XCTAssertEqual(effect.choices.filter(\.selected).map(\.value), ["Blazing"])
        XCTAssertEqual(effect.choices(in: "enchantment").count + effect.choices(in: "curse").count,
                       effect.choices.count)
        XCTAssertEqual(effect.caption, "Matches any one of 1 effect.")

        let copies = try fixtureForm("editor-change-count").stack.copyDepth
        XCTAssertTrue(copies.visible)
        XCTAssertTrue(copies.enabled)
        XCTAssertEqual(copies.value, 6)
        XCTAssertEqual(copies.index, 4)
        XCTAssertEqual(copies.valueLabel, "Copies within first 6 floors")

        let refused = try fixtureForm("editor-save-refused")
        XCTAssertEqual(refused.title, "Rat Skull")
        XCTAssertNil(refused.preview)
        XCTAssertEqual(refused.errors,
                       ["This trinket is already required. Each trinket appears only once in the deck."])
        XCTAssertFalse(refused.canSave)
    }

    /// The resin chip's sheet edits the query's condition: its amount and
    /// the Mage's wand in the resin section, its donors through the
    /// placement controls.
    func testTheResinChipsSheetReadsTheQuerysCondition() throws {
        let form = try fixtureForm("editor-resin-open")
        XCTAssertEqual(form.origin, .resin)
        XCTAssertEqual(form.mode, .edit)
        XCTAssertTrue(form.resinPicked)
        XCTAssertEqual(form.title, "Arcane Resin")
        XCTAssertNil(form.preview)
        XCTAssertEqual(form.item.value, RequirementSheet.arcaneResin)
        XCTAssertEqual(form.item.selected?.isArcaneResin, true)
        XCTAssertTrue(form.resin.visible)
        XCTAssertEqual(form.resin.label, "Minimum resin")
        XCTAssertTrue(form.resin.auto)
        XCTAssertEqual(form.resin.modes.map(\.value), [false, true])
        XCTAssertEqual(form.resin.modes.map(\.label), ["Amount", "Auto"])
        XCTAssertEqual(form.resin.caption,
                       "Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.")
        XCTAssertEqual(form.resin.range, 1...65_535)
        XCTAssertTrue(form.resin.includeMageWand.visible)
        XCTAssertTrue(form.resin.includeMageWand.value)
        XCTAssertEqual(form.resin.includeMageWand.label, "Include Mage’s starting wand")
        XCTAssertEqual(form.resin.includeMageWand.caption,
                       "Add 2 resin from the Magic Missile wand recovered with Wand Preservation when imbuing another wand. The preserved wand is +0, regardless of the staff’s level.")
        XCTAssertEqual(form.resin.amount, 2)
        XCTAssertEqual(form.resin.wholeAmount, 2)
        XCTAssertEqual(form.resin.amountText, "2")
        XCTAssertEqual(form.uncursed.label, "Require uncursed wands")
        XCTAssertFalse(form.uncursed.value)
        XCTAssertEqual(form.source.value, "chest")
        XCTAssertEqual(form.floorLimit.label, "Limit wands to a floor")
        XCTAssertFalse(form.upgrade.visible)
        XCTAssertFalse(form.effect.visible)
        XCTAssertFalse(form.stack.visible)
        XCTAssertFalse(form.excludeResin.visible)

        let invalid = try fixtureForm("editor-resin-amount-invalid")
        XCTAssertNil(invalid.resin.amount)
        XCTAssertNil(invalid.resin.wholeAmount)
        XCTAssertEqual(invalid.resin.amountText, "")
        XCTAssertEqual(invalid.errors, ["Enter an amount from 1 to 65535."])
        XCTAssertFalse(invalid.canSave)
    }

    /// A save's rows, focus and resin outcome read — and the rows survive
    /// the app's codec.
    func testSavedAnswersRead() throws {
        let sent = [try requirement(1, kind: .wand), try requirement(2, item: "rat_skull")]
        let saves = try ["editor-save", "editor-resin-save-set", "editor-resin-save-clear"].map { name in
            try XCTUnwrap(SheetSaved(json: try XCTUnwrap(try response(name)["saved"] as? [String: Any], name),
                                     sent: sent), name)
        }
        XCTAssertEqual(saves[0].rows.map(\.key), [1, 2, 3, 4])
        XCTAssertEqual(saves[0].focus, 3)
        XCTAssertEqual(saves[0].resin, .unchanged)
        XCTAssertEqual(saves[0].rows[2].effect, .oneOf(["Blazing"]))
        // Turning the wand chip into Arcane Resin takes the chip off the board.
        XCTAssertEqual(saves[1].rows.map(\.key), [2])
        XCTAssertNil(saves[1].focus)
        XCTAssertEqual(saves[1].resin, .set(try XCTUnwrap(BoardResin(
            amount: 4, auto: false, filter: ArcaneResinFilter(uncursed: true, maximumDepth: 14)))))
        XCTAssertEqual(saves[2].rows.map(\.key), [1, 2, 3])
        XCTAssertEqual(saves[2].resin, .clear)
        for save in saves {
            XCTAssertTrue(save.changed)
            XCTAssertTrue(save.rekeyed.isEmpty)
            for row in save.rows {
                let written = ResultsExport.encodeRow(row)
                XCTAssertEqual(ResultsExport.decodeRow(written), row)
            }
        }
    }

    // MARK: - Through the engine

    /// Every change the dialogs send is one the core reads, shown or hidden;
    /// out-of-range numbers and text that is no amount included.
    func testEveryChangeIsOneTheCoreReads() throws {
        let opened = try XCTUnwrap(RequirementSheet.open(rows: []))
        let changes: [SheetChange] = [
            .category("armor"), .weaponType("melee"), .item(nil), .item("spear"),
            .item(RequirementSheet.arcaneResin), .tierMode("at_least"), .tier(300), .upgradeMode("exact"),
            .upgrade(-1), .effectMode("specific"), .toggleEffect("Blazing"), .uncursed(true),
            .source("locked_chest"), .source(nil), .floorLimitEnabled(true), .floorLimit(9),
            .excludeResin(true), .transmutationsEnabled(true), .transmutations(3), .selectTrinket(true),
            .count(2), .copyDepthEnabled(true), .copyDepth(6), .countLevels(true), .total(5),
            .resinAuto(true), .resinAmount(nil), .resinAmount(4), .resinAmount(2.5),
            .resinAmount(.infinity), .includeMageWand(true),
        ]
        for change in changes {
            XCTAssertNotNil(opened.changing(change), "the core did not read \(change)")
        }
    }

    /// A change to a control the form hides changes nothing, and numbers
    /// are clamped into range.
    func testHiddenControlsChangeNothing() throws {
        let rows = [try requirement(1, item: "spear")]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, key: 1))
        XCTAssertFalse(opened.form.tier.visible)
        XCTAssertEqual(opened.changing(.tier(4)), opened)
        XCTAssertEqual(opened.changing(.count(9))?.form.stack.count, 3)
    }

    /// A new ring stack counting its levels lands as one combined level.
    func testANewRingStackCountsLevelsThroughTheEngine() throws {
        let rows = [try requirement(1, kind: .wand)]
        let sheet = try moved(try XCTUnwrap(RequirementSheet.open(rows: rows)), [
            .category("ring"), .item("ring_might"), .count(2), .countLevels(true), .total(6),
        ])
        XCTAssertEqual(sheet.form.title, "Ring of Might")
        XCTAssertFalse(sheet.form.upgrade.visible)
        XCTAssertFalse(sheet.form.stack.copyDepth.visible)
        XCTAssertEqual(sheet.form.stack.countLevels.value, 6)
        let saved = try landed(sheet.save(onto: rows))
        XCTAssertTrue(saved.changed)
        XCTAssertEqual(saved.focus, 2)
        XCTAssertEqual(saved.rows.map(\.key), [1, 2, 3])
        XCTAssertEqual(saved.rows.dropFirst().map(\.levelSum?.atLeast), [6, 6])
        XCTAssertEqual(RequirementBoard.of(saved.rows).chip(2)?.stack.total, 6)
        XCTAssertNoThrow(try SearchRequest(requirements: saved.rows))
    }

    /// A new chip takes the key the core mints, with its stack's copies; an
    /// unchanged chip saves back the very rows.
    func testSavesMintKeysAndAnUnchangedSaveChangesNothing() throws {
        let wand = try requirement(1, kind: .wand)
        let sheet = try moved(try XCTUnwrap(RequirementSheet.open(rows: [wand])), [
            .weaponType("thrown"), .upgradeMode("at_least"), .upgrade(2), .count(2),
            .copyDepthEnabled(true), .copyDepth(6),
        ])
        XCTAssertEqual(sheet.form.upgrade.valueLabel, "+2 or higher")
        let saved = try landed(sheet.save(onto: [wand]))
        XCTAssertTrue(saved.changed)
        XCTAssertEqual(saved.rows.map(\.key), [1, 2, 3])
        XCTAssertEqual(saved.focus, 2)
        let board = RequirementBoard.of(saved.rows)
        XCTAssertEqual(board.chip(2)?.stack.count, 2)
        XCTAssertEqual(board.chip(2)?.stack.copyDepth, 6)
        XCTAssertNoThrow(try SearchRequest(requirements: saved.rows))

        let chip = [try requirement(1, item: "ring_might", upgrade: 2, upgradeMatch: .exactly),
                    try requirement(7, item: "ring_might", maximumDepth: 9)]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: chip, key: 1))
        XCTAssertEqual(opened.form.stack.count, 2)
        XCTAssertEqual(opened.form.stack.copyDepth.value, 9)
        let again = try landed(opened.save(onto: chip))
        XCTAssertFalse(again.changed)
        XCTAssertEqual(again.focus, 1)
        XCTAssertEqual(again.rows, chip)
    }

    /// A trinket another ordinary row names cannot be saved: the form says
    /// so before the save, and the save is refused with it.
    func testADuplicateTrinketIsRefused() throws {
        let rows = [try requirement(1, item: "rat_skull")]
        let sheet = try moved(try XCTUnwrap(RequirementSheet.open(rows: rows)), [
            .category("trinket"), .item("rat_skull"),
        ])
        let message = "This trinket is already required. Each trinket appears only once in the deck."
        XCTAssertEqual(sheet.form.errors, [message])
        XCTAssertFalse(sheet.form.canSave)
        XCTAssertNil(sheet.form.preview)
        let refused = try turnedBack(sheet.save(onto: rows))
        XCTAssertEqual(refused.form.errors, [message])
    }

    /// Arcane Resin picked on a wand chip's sheet sets the query's resin and
    /// takes the chip off the board.
    func testArcaneResinReplacesTheWandChip() throws {
        let rows = [try requirement(1, kind: .wand), try requirement(2, item: "ring_might")]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, key: 1, offerResin: true))
        XCTAssertTrue(opened.form.item.options.contains(where: \.isArcaneResin))
        let picked = try moved(opened, [.item(RequirementSheet.arcaneResin)])
        XCTAssertTrue(picked.form.resinPicked)
        XCTAssertEqual(picked.form.title, "Arcane Resin")
        XCTAssertEqual(picked.form.resin.amount, 2)
        XCTAssertTrue(picked.form.uncursed.value, "donors are uncursed by default")
        let sheet = try moved(picked, [.resinAmount(4), .floorLimitEnabled(true), .floorLimit(9)])
        let saved = try landed(sheet.save(onto: rows))
        XCTAssertEqual(saved.rows.map(\.key), [2])
        XCTAssertNil(saved.focus)
        XCTAssertEqual(saved.resin, .set(try XCTUnwrap(BoardResin(
            amount: 4, auto: false, filter: ArcaneResinFilter(uncursed: true, maximumDepth: 9)))))

        // An amount that is no whole number keeps the sheet from saving.
        let empty = try moved(picked, [.resinAmount(nil)])
        XCTAssertEqual(empty.form.errors, ["Enter an amount from 1 to 65535."])
        XCTAssertFalse(empty.form.canSave)
        XCTAssertEqual(try turnedBack(empty.save(onto: rows)).form.errors, empty.form.errors)
    }

    /// An Auto amount saves as Auto: the core writes `"amount": "auto"`, and
    /// the query keeps no amount of its own.
    func testAnAutoResinSavesAsAuto() throws {
        let rows = [try requirement(1, kind: .wand), try requirement(2, item: "ring_might")]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, key: 1, offerResin: true))
        let auto = try moved(opened, [.item(RequirementSheet.arcaneResin), .resinAuto(true)])
        XCTAssertTrue(auto.form.resin.auto)
        XCTAssertTrue(auto.form.errors.isEmpty)
        XCTAssertTrue(auto.form.canSave)
        let saved = try landed(auto.save(onto: rows))
        XCTAssertEqual(saved.rows.map(\.key), [2])
        XCTAssertEqual(saved.resin, .set(try XCTUnwrap(BoardResin(amount: 0, auto: true, filter: ArcaneResinFilter()))))

        // The same answer as written, its filter's unset fields null.
        let text = #"{"changed": true, "rows": [], "rekeyed": [], "focus": null, "resin": {"set": {"amount": "auto", "filter": {"uncursed": false, "max_depth": null, "source": "locked_chest", "include_mage_wand": true}}}}"#
        let object = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any])
        let written = try XCTUnwrap(SheetSaved(json: object, sent: rows))
        XCTAssertEqual(written.rows, [])
        XCTAssertNil(written.focus)
        XCTAssertEqual(written.resin, .set(try XCTUnwrap(BoardResin(
            amount: 0, auto: true,
            filter: ArcaneResinFilter(uncursed: false, source: .lockedChest, includeMageWand: true)))))
    }

    /// The resin chip saved as a wand adds the wand and clears the query's
    /// resin; an Auto condition opens as Auto.
    func testTheResinChipSavedAsAWandClearsTheResin() throws {
        let rows = [try requirement(1, kind: .wand), try requirement(2, item: "ring_might")]
        let resin = try XCTUnwrap(BoardResin(amount: 0, auto: true, filter: ArcaneResinFilter(includeMageWand: true)))
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, resin: resin, openResin: true))
        XCTAssertEqual(opened.form.origin, .resin)
        XCTAssertTrue(opened.form.resin.auto)
        XCTAssertTrue(opened.form.resin.includeMageWand.value)
        let frost = try moved(opened, [.item("wand_frost")])
        XCTAssertFalse(frost.form.resinPicked)
        XCTAssertEqual(frost.form.title, "Wand of Frost")
        let saved = try landed(frost.save(onto: rows))
        XCTAssertEqual(saved.rows.map(\.key), [1, 2, 3])
        XCTAssertEqual(saved.rows.last?.item?.id, "wand_frost")
        XCTAssertEqual(saved.focus, 3)
        XCTAssertEqual(saved.resin, .clear)
    }

    /// The form words what the dialogs used to word themselves — help texts,
    /// section labels — and says when a slider, the effect grid or a help
    /// text shows.
    func testTheFormWordsItsSectionsAndSaysWhatShows() throws {
        let opened = try XCTUnwrap(RequirementSheet.open(rows: []))
        XCTAssertFalse(opened.form.tier.valueVisible)
        XCTAssertEqual(opened.form.stack.label, "Total item count")
        let tier = try moved(opened, [.tierMode("at_least")]).form.tier
        XCTAssertTrue(tier.valueVisible)
        XCTAssertEqual(tier.valueLabel, "Tier 3 or higher")
        XCTAssertTrue(try moved(opened, [.upgradeMode("exact")]).form.upgrade.valueVisible)

        let armor = try moved(opened, [.category("armor")]).form.effect
        XCTAssertEqual(armor.label, "Glyph")
        XCTAssertFalse(armor.choicesVisible)
        XCTAssertTrue(try moved(opened, [.category("armor"), .effectMode("specific")]).form.effect.choicesVisible)

        let wand = try moved(opened, [.category("wand")]).form.excludeResin
        XCTAssertTrue(wand.visible)
        XCTAssertEqual(wand.label, "Exclude from Auto resin")
        XCTAssertEqual(wand.caption, "Keep this wand without budgeting resin to upgrade it. Useful for imbuing: resin upgrades do not transfer to the staff. Extra copies are reserved for reforging and never need Auto resin.")

        // The transmutation limit's help describes the limit, so it shows
        // only while the limit is on.
        let trinket = try moved(opened, [.category("trinket"), .item("rat_skull")])
        XCTAssertTrue(trinket.form.selectTrinket.visible)
        XCTAssertEqual(trinket.form.selectTrinket.caption,
                       "Applies after the first brewing opportunity. If several alternatives are offered, no trinket is chosen.")
        XCTAssertTrue(trinket.form.transmutations.visible)
        XCTAssertFalse(trinket.form.transmutations.captionVisible)
        XCTAssertNotNil(trinket.form.transmutations.caption)
        let limited = try moved(trinket, [.transmutationsEnabled(true)]).form.transmutations
        XCTAssertTrue(limited.captionVisible)
        XCTAssertEqual(limited.valueLabel, "At most 1")

        // The combined level's help explains its switch, so it shows while
        // the switch is off too.
        let rings = try moved(opened, [.category("ring"), .item("ring_might"), .count(2)]).form.stack.countLevels
        XCTAssertTrue(rings.visible)
        XCTAssertFalse(rings.enabled)
        XCTAssertTrue(rings.captionVisible)
        XCTAssertEqual(rings.caption, "Each item counts its upgrade plus one, and spare items may go unused.")
    }

    /// A resin sheet opened on a query without resin adds one: Add, and no
    /// Remove, as a new sheet's chrome is.
    func testAResinSheetOnAQueryWithoutResinIsNew() throws {
        let fixture = try fixtureForm("editor-resin-open-new")
        XCTAssertEqual(fixture.mode, .new)
        XCTAssertEqual(fixture.origin, .new)
        XCTAssertTrue(fixture.resinPicked)

        let rows = [try requirement(1, item: "wand_frost")]
        let fresh = try XCTUnwrap(RequirementSheet.open(rows: rows, openResin: true))
        XCTAssertEqual(fresh.form.mode, .new)
        XCTAssertEqual(fresh.form.origin, .new)
        XCTAssertTrue(fresh.form.resinPicked)
        XCTAssertEqual(fresh.form.title, "Arcane Resin")
        XCTAssertFalse(fresh.form.resin.includeMageWand.value)
        let resin = try XCTUnwrap(BoardResin(amount: 4, auto: false, filter: ArcaneResinFilter()))
        let existing = try XCTUnwrap(RequirementSheet.open(rows: rows, resin: resin, openResin: true))
        XCTAssertEqual(existing.form.mode, .edit)
        XCTAssertEqual(existing.form.origin, .resin)
    }

    /// The resin chip saved untouched leaves the query's resin as it is —
    /// a donor floor on an empty boss floor included, which the slider
    /// shows as the floor below — while a changed one sets it.
    func testAnUntouchedResinSheetLeavesTheQuerysResin() throws {
        let saved = try XCTUnwrap(try response("editor-resin-save-untouched")["saved"] as? [String: Any])
        XCTAssertEqual(try XCTUnwrap(SheetSaved(json: saved, sent: [])).resin, .unchanged)

        let rows = [try requirement(1, item: "wand_frost")]
        let resin = try XCTUnwrap(BoardResin(amount: 4, auto: false, filter: ArcaneResinFilter(
            uncursed: true, maximumDepth: 5, source: .lockedChest, includeMageWand: true)))
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, resin: resin, openResin: true))
        XCTAssertEqual(opened.form.floorLimit.value, 4)
        let untouched = try landed(opened.save(onto: rows))
        XCTAssertFalse(untouched.changed)
        XCTAssertEqual(untouched.rows, rows)
        XCTAssertEqual(untouched.resin, .unchanged)
        let flipped = try landed(try moved(opened, [.resinAuto(true), .resinAuto(false)]).save(onto: rows))
        XCTAssertEqual(flipped.resin, .unchanged)

        let more = try landed(try moved(opened, [.resinAmount(6)]).save(onto: rows))
        XCTAssertEqual(more.resin, .set(try XCTUnwrap(BoardResin(amount: 6, auto: false, filter: ArcaneResinFilter(
            uncursed: true, maximumDepth: 4, source: .lockedChest, includeMageWand: true)))))
    }

    /// An artifact's upgrade has no control on the sheet, yet a save keeps
    /// it: untouched, the rows come back as they were; changed, it stays.
    func testASaveKeepsWhatTheSheetCannotShow() throws {
        let sandals = try XCTUnwrap(ItemCatalog.findById("sandals_of_nature"))
        let rows = [try ItemRequirement(key: 1, item: sandals, upgrade: 5, kind: .artifact,
                                        upgradeMatch: .exactly, maximumDepth: 19)]
        let opened = try XCTUnwrap(RequirementSheet.open(rows: rows, key: 1))
        XCTAssertFalse(opened.form.upgrade.visible)
        let untouched = try landed(opened.save(onto: rows))
        XCTAssertFalse(untouched.changed)
        XCTAssertEqual(untouched.rows, rows)
        XCTAssertEqual(untouched.focus, 1)

        let refloored = try landed(try moved(opened, [.floorLimit(14)]).save(onto: rows))
        XCTAssertTrue(refloored.changed)
        XCTAssertEqual(refloored.rows.first?.upgrade, 5)
        XCTAssertEqual(refloored.rows.first?.upgradeMatch, .exactly)
        XCTAssertEqual(refloored.rows.first?.maximumDepth, 14)

        // A hand-written row the sheet had to repair as it opened is written
        // back repaired even untouched.
        let repaired = try XCTUnwrap(try response("editor-save-untouched-repairs")["saved"] as? [String: Any])
        let answer = try XCTUnwrap(SheetSaved(json: repaired, sent: []))
        XCTAssertTrue(answer.changed)
        XCTAssertEqual(answer.rows.map(\.key), [1, 2])
        XCTAssertEqual(answer.rows.map(\.excludeResin), [false, false])
    }

    /// A draft the core cannot read — one from another version — is no
    /// answer, and the dialog keeps what it shows.
    func testAnUnreadableDraftIsNoAnswer() throws {
        let opened = try XCTUnwrap(RequirementSheet.open(rows: []))
        let stale = RequirementSheet(draft: "{\"v\":99}", form: opened.form)
        XCTAssertNil(stale.changing(.uncursed(true)))
        XCTAssertNil(stale.save(onto: []))
    }
}
