// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniBindings
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The bridge to the shared requirement editor. The editor's rules have their
 * own tests in the core (`crates/seedfinder-core/src/editor/`); these pin the
 * binding, the row codec, and the decoding of its answers, through the real
 * engine.
 */
class RequirementEditorTest {
    init { PackagedCatalog.install() }

    private fun find(id: String): CatalogItem = requireNotNull(ItemCatalog.findById(id)) { id }

    /** The core's golden request/answer pairs (docs/requirement-editor.md, "Golden fixtures"). */
    private val fixtures: File = generateSequence(File("").absoluteFile) { it.parentFile }
        .map { File(it, "crates/seedfinder-core/tests/fixtures/editor") }
        .first { it.isDirectory }

    private fun fixture(name: String) = JSONObject(File(fixtures, "$name.json").readText())

    /** A JSON value with object key order and number spelling set aside. */
    private fun canonical(value: Any?): Any? = when (value) {
        is JSONObject -> value.keys().asSequence().associateWith { canonical(value.get(it)) }
        is JSONArray -> List(value.length()) { canonical(value.get(it)) }
        JSONObject.NULL -> null
        is Int, is Long -> (value as Number).toLong()
        is Number -> value.toDouble()
        else -> value
    }

    @Test fun everyGoldenFixtureIsAnsweredTheSameThroughTheBinding() {
        val files = fixtures.listFiles { file -> file.extension == "json" }.orEmpty().sortedBy { it.name }
        assertTrue(files.size >= 30)
        for (file in files) {
            val fixture = JSONObject(file.readText())
            // A request stored as a string is sent as that very text.
            val request = fixture.get("request").let { if (it is String) it else it.toString() }.toByteArray()
            val answer = when (val envelope = fixture.getString("envelope")) {
                "requirement_board" -> JniBindings.requirementBoard(request)
                "requirement_editor" -> JniBindings.requirementEditor(request)
                else -> error("${file.name}: unknown envelope $envelope")
            }
            assertEquals(
                file.name,
                canonical(fixture.getJSONObject("response")),
                canonical(JSONObject(String(answer, Charsets.UTF_8))),
            )
        }
    }

    @Test fun theBoardTourDecodesIntoWhatTheBoardDraws() {
        val board = BoardView.decode(fixture("board-tour").getJSONObject("response"))
        assertEquals(4, board.ordinaryCount)
        assertEquals(1, board.blanketCount)
        assertTrue(board.problems.isEmpty())
        val (rings, melee, wands, skull, armor) = board.items

        assertEquals(listOf(1L), rings.members)
        assertEquals(3, rings.count)
        assertEquals(BadgeView("×3", "×3", "3 of the same kind"), rings.countBadge)
        assertNull(rings.totalBadge)
        val might = rings.chips.single()
        assertEquals("Ring of Might", might.name)
        assertEquals(find("ring_might"), might.item)
        assertEquals(listOf(TagView("+2", upgrade = true)), might.tags)
        assertEquals(setOf(4L, 5L, 6L, 7L), might.refuse.keys)
        assertEquals("Copies can only be grouped with the same item type.", might.refuse.getValue(4))
        assertTrue(might.join.isEmpty())

        val anyMelee = melee.chips.single()
        assertEquals("Any melee", anyMelee.name)
        assertNull(anyMelee.item)
        assertEquals(ItemKind.MELEE_WEAPON, anyMelee.kind)
        assertEquals(
            listOf(TagView("T3+", upgrade = false), TagView("+2↑", upgrade = true), TagView("F≤9", upgrade = false)),
            anyMelee.tags,
        )
        assertTrue(anyMelee.effect!!.anyEnchantment)
        assertEquals("any enchantment", anyMelee.effect!!.label)
        assertTrue(anyMelee.uncursed)
        assertEquals(setOf(5L, 6L, 7L), anyMelee.join)
        assertEquals("Any Tier 3+ melee weapon, +2 or higher, any enchantment, uncursed, floors 1–9", anyMelee.description)

        assertEquals(1, wands.cluster)
        assertEquals(listOf(5L, 6L), wands.members)
        assertTrue(wands.chips.all { it.canDetach })
        assertEquals(listOf(TagView("No resin", upgrade = false)), wands.chips[1].trailingTags)

        assertEquals(find("rat_skull"), skull.chips.single().item)
        assertEquals(listOf(TagView("Transmute ≤3", upgrade = false)), skull.chips.single().tags)

        assertTrue(armor.blanket)
        assertEquals(listOf("Viscosity", "Brimstone"), armor.chips.single().effect!!.effects)
        assertEquals("effect: Viscosity/Brimstone", armor.chips.single().effect!!.label)

        val resin = board.resin!!
        assertEquals("Arcane Resin", resin.name)
        assertEquals(listOf("Auto", "Mage +2"), resin.tags.map { it.text })
        assertTrue(resin.uncursed)
        assertTrue(resin.description.startsWith("Arcane Resin, Auto"))
    }

    @Test fun problemsDecodeInOrderWithTheRowsTheyBlame() {
        val board = BoardView.decode(fixture("board-problems").getJSONObject("response"))
        assertEquals(
            listOf(
                RequirementProblem("Requirement floor must be 1 through 24.", listOf(2L)),
                RequirementProblem("A blanket requirement cannot request extra copies, combined levels, or trinket selection.", listOf(4L)),
                RequirementProblem("A stack must share one combined level.", listOf(1L, 3L)),
            ),
            board.problems,
        )
        assertEquals("Requirement floor must be 1 through 24.", board.itemOf(2)!!.chips.single().problem)
        assertEquals("A stack must share one combined level.", board.itemOf(1)!!.chips.single().problem)
    }

    @Test fun answersDecodeRowsKeysAndRefusals() {
        val saved = BoardAnswer.decode(fixture("board-save-new").getJSONObject("response"))
        assertEquals(listOf(1L, 5L, 6L), saved.rows!!.map { it.key })
        assertEquals(ItemKind.THROWN_WEAPON, saved.rows!![1].kind)
        assertEquals(UpgradeMatch.AT_LEAST, saved.rows!![1].upgradeMatch)
        assertEquals(listOf(null, 1, 1), saved.rows!!.map { it.identityGroup })
        assertEquals(6, saved.rows!![2].maximumDepth)
        assertEquals(7L, saved.nextKey)
        assertEquals(5L, saved.focus)
        assertNull(saved.refused)

        val refused = BoardAnswer.decode(fixture("board-join-refused").getJSONObject("response"))
        assertNull(refused.rows)
        assertEquals("Copies can only be grouped with the same item type.", refused.refused)

        val repaired = BoardAnswer.decode(fixture("board-key-repair").getJSONObject("response"))
        assertEquals(mapOf(0L to 5L, 4L to 6L), repaired.rekeyed)
        assertEquals(listOf(null, null, 1, 1), repaired.rows!!.map { it.alternativeGroup })
    }

    @Test fun errorAnswersAndPanicsNeverBecomeBoards() {
        val failure = assertThrows(IllegalStateException::class.java) {
            RequirementEditor.answer(JniBindings.requirementBoard("{\"rows\": [".toByteArray()))
        }
        assertTrue(failure.message!!.startsWith("invalid request"))
        assertThrows(IllegalStateException::class.java) {
            RequirementEditor.answer(JniBindings.requirementBoard(byteArrayOf(0xff.toByte())))
        }
        // A row this app cannot hold is reported, never adopted.
        val unreadable = JSONObject(
            """{"changed":true,"rows":[{"key":1,"kind":"artifact","identity_group":1}],"next_key":2,"rekeyed":[],
            "focus":null,"refused":null,"items":[],"counts":{"ordinary":0,"blanket":0},"problems":[],"resin":null}""",
        )
        assertThrows(IllegalStateException::class.java) { BoardAnswer.decode(unreadable) }
    }

    @Test fun rowsRoundTripThroughTheEditorInItsOwnSpelling() {
        val rows = listOf(
            ItemRequirement(1, find("wand_fireblast"), 3),
            ItemRequirement(
                2, null, 2, kind = ItemKind.MELEE_WEAPON, upgradeMatch = UpgradeMatch.AT_LEAST,
                tier = 3, tierMatch = TierMatch.AT_LEAST, effect = EffectFilter.AnyEnchantment,
                requireUncursed = true, maximumDepth = 9, source = ScoutItemSource.LOCKED_CHEST,
            ),
            ItemRequirement(
                3, null, 0, kind = ItemKind.THROWN_WEAPON, upgradeMatch = UpgradeMatch.ANY,
                tier = 4, tierMatch = TierMatch.EXACT, effect = EffectFilter.OneOf(listOf("Blazing", "Lucky")),
            ),
            ItemRequirement(4, find("ring_might"), 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
            ItemRequirement(5, find("ring_might"), 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
            ItemRequirement(
                6, null, 1, kind = ItemKind.ARMOR, tier = 4, tierMatch = TierMatch.AT_MOST,
                effect = EffectFilter.OneOf(listOf("Brimstone")), identityGroup = 2,
            ),
            ItemRequirement(7, null, 0, kind = ItemKind.ARMOR, upgradeMatch = UpgradeMatch.ANY, identityGroup = 2, maximumDepth = 4),
            ItemRequirement(8, find("wand_frost"), 2, alternativeGroup = 1, excludeResin = true),
            ItemRequirement(9, null, 1, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.AT_LEAST, alternativeGroup = 1),
            ItemRequirement(10, find("rat_skull"), 0, upgradeMatch = UpgradeMatch.ANY, trinketTransmutations = 3),
            ItemRequirement(11, ItemCatalog.trinkets.first { it.id != "rat_skull" }, 0, upgradeMatch = UpgradeMatch.ANY, selectTrinket = true),
            ItemRequirement(12, find("ethereal_chains"), 5, artifactTransmutations = 2),
            ItemRequirement(13, null, 3, kind = ItemKind.WAND, blanket = true, source = ScoutItemSource.WANDMAKER_REWARD),
        )
        val sent = RequirementEditor.encodeRows(rows)
        val answer = RequirementEditor.answer(JniBindings.requirementBoard(JSONObject().put("rows", sent).toString().toByteArray()))
        assertTrue(!answer.getBoolean("changed"))
        assertEquals(canonical(sent), canonical(answer.getJSONArray("rows")))
        assertEquals(rows, RequirementEditor.decodeRows(answer.getJSONArray("rows")))
        assertNull(RequirementEditor.board(rows).rows)
        assertTrue(RequirementEditor.view(rows).problems.isEmpty())
    }

    @Test fun aJoinIsAdoptedAndARefusedOneLeavesTheListAlone() {
        val energy = find("ring_energy")
        val rows = listOf(
            ItemRequirement(1, energy, 4),
            ItemRequirement(2, energy, 0, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(3, energy, 0, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(4, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(5, find("wand_frost"), 2),
        )
        val wand = RequirementEditor.view(rows).itemOf(4)!!.chips.single()
        assertEquals(setOf(5L), wand.join)
        assertEquals(setOf(1L), wand.refuse.keys)

        val refused = RequirementEditor.board(rows, listOf(BoardEdit.Join(source = 4, target = 1)))
        assertNull(refused.rows)
        assertEquals(wand.refuse.getValue(1), refused.refused)

        val joined = RequirementEditor.board(rows, listOf(BoardEdit.Join(source = 4, target = 5)))
        assertEquals(4L, joined.focus)
        assertEquals(listOf(5L, 4L), joined.board.itemOf(4)!!.members)
        val (frost, anyWand) = joined.rows!!.takeLast(2)
        assertEquals(listOf(5L, 4L), listOf(frost.key, anyWand.key))
        assertEquals(frost.alternativeGroup, anyWand.alternativeGroup)

        val detached = RequirementEditor.board(joined.rows!!, listOf(BoardEdit.Detach(4)))
        assertTrue(detached.rows!!.all { it.alternativeGroup == null })
        val removed = RequirementEditor.board(joined.rows!!, listOf(BoardEdit.Remove(4)))
        assertEquals(listOf(1L, 2L, 3L, 5L), removed.rows!!.map { it.key })
    }

    @Test fun savingKeepsAnUnchangedStackAndAppendsANewChip() {
        val might = ItemRequirement(0, find("ring_might"), 2)
        val stacked = RequirementEditor.board(
            emptyList(), listOf(BoardEdit.Save(null, might, count = 3, total = null, copyDepth = 9)), nextKey = 1,
        ).rows!!
        assertEquals(listOf(1L, 2L, 3L), stacked.map { it.key })
        assertEquals(listOf(null, 9, 9), stacked.map { it.maximumDepth })

        // Saving what is already there changes nothing, so a refine can resume.
        val unchanged = RequirementEditor.board(stacked, listOf(BoardEdit.Save(1, stacked[0], 3, null, 9)))
        assertNull(unchanged.rows)
        assertEquals(1L, unchanged.focus)

        val wand = ItemRequirement(0, find("wand_frost"), 2)
        val added = RequirementEditor.board(stacked, listOf(BoardEdit.Save(null, wand, 1, null, null)), nextKey = 10)
        assertEquals(wand.copy(key = 10), added.rows!!.last())
        assertEquals(11L, added.nextKey)
    }

    @Test fun aStackedClusterMemberSavedAsATrinketIsRefused() {
        val spear = ItemRequirement(0, find("spear"), 2)
        val stacked = RequirementEditor.board(
            emptyList(), listOf(BoardEdit.Save(null, spear, count = 2, total = null, copyDepth = null)), nextKey = 1,
        ).rows!! + ItemRequirement(3, find("mace"), 2)
        val cluster = RequirementEditor.board(stacked, listOf(BoardEdit.Join(source = 3, target = 1))).rows!!
        assertEquals(2, RequirementEditor.view(cluster).items.single().count)

        val skull = ItemRequirement(3, find("rat_skull"), 0, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1)
        val answer = RequirementEditor.board(cluster, listOf(BoardEdit.Save(3, skull, 1, null, null)))
        assertNull(answer.rows)
        assertEquals("Copies can only be grouped with the same item type.", answer.refused)
    }

    @Test fun everyEditIsReadByTheEditor() {
        val haste = find("ring_haste")
        val rows = listOf(
            ItemRequirement(1, haste, 1),
            ItemRequirement(2, haste, 0, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(3, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1),
            ItemRequirement(4, find("wand_frost"), 2, alternativeGroup = 1),
        )
        val edits = listOf(
            BoardEdit.Normalize, BoardEdit.Join(1, 3), BoardEdit.Detach(3), BoardEdit.Remove(4), BoardEdit.RemoveItem(1),
            BoardEdit.SetCount(1, 3), BoardEdit.SetTotal(1, 4), BoardEdit.SetTotal(1, null), BoardEdit.ToggleLevels(1),
            BoardEdit.SetCopyDepth(1, 10), BoardEdit.SetCopyDepth(1, null),
            BoardEdit.Save(1, rows[0], count = 2, total = 3, copyDepth = null), BoardEdit.Save(null, rows[3], 1, null, null),
        )
        for (edit in edits) RequirementEditor.board(rows, listOf(edit), ResinCondition.of(4, auto = false, ArcaneResinFilter()))
        val floored = RequirementEditor.board(rows, listOf(BoardEdit.SetCopyDepth(1, 10))).rows!!
        // Floor 10 is an empty boss floor; the copies keep to floor 9.
        assertEquals(9, floored[1].maximumDepth)
    }

    @Test fun loadedListsAreKeyedOnInTheCanonicalEncoding() {
        val frost = find("wand_frost")
        val labelled = listOf(
            ItemRequirement(7, frost, 3, identityGroup = 1),
            ItemRequirement(7, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, identityGroup = 1),
        )
        // A stack of a named item is written as plain repeats.
        val loaded = RequirementEditor.loaded(labelled, firstKey = 40)
        assertEquals(listOf(40L, 41L), loaded.map { it.key })
        assertEquals(listOf(null, null), loaded.map { it.identityGroup })
        assertEquals(listOf(frost, frost), loaded.map { it.item })
        // Built-in presets are already canonical: loading one changes only its keys.
        for (preset in BuiltInPresets.all) {
            val requirements = preset.query.requirements
            assertEquals(preset.name, requirements.mapIndexed { index, it -> it.copy(key = 1L + index) }, RequirementEditor.loaded(requirements, 1))
        }
    }

    @Test fun theSheetFormDecodesEveryControl() {
        val sheet = EditorSheet.decode(fixture("editor-open-row").getJSONObject("response"))
        val form = sheet.form
        assertFalse(form.adding)
        assertEquals(1L, form.rowKey)
        assertEquals("Ring of Might", form.title)
        assertEquals(ItemKind.RING, form.kind)
        assertEquals("ring", form.category.value)
        assertEquals(listOf("Weapon", "Armor", "Wand", "Ring", "Trinket", "Artifact"), form.category.options.map { it.label })
        assertEquals("ring_might", form.item.value)
        assertEquals(SheetOption<String?>(null, "Any ring", null, hidden = false), form.item.options.first())
        // Counting levels speaks for the rings' upgrades; a named ring has no tier.
        assertFalse(form.upgrade.visible)
        assertFalse(form.tier.visible)
        assertNull(form.source.value)
        assertEquals("Any", form.source.label)
        assertEquals((1..24).filterNot { it in setOf(5, 10, 15) }, form.floorLimit.options.map { it.value })
        assertEquals(SheetFloors(true, false, 4, form.floorLimit.options, "Limit this item to a floor", "Within first 4 floors"), form.floorLimit)
        assertEquals(2, form.stack.count)
        assertEquals("×2", form.stack.valueLabel)
        assertEquals(SheetStepper(true, true, 3, 1, 8, "Count levels together", null, "≥ 3 across up to 2"), form.stack.countLevels)
        assertFalse(form.stack.copyDepth.visible)
        assertEquals(listOf("up to 2 — levels add to ≥ 3"), form.preview!!.relations)
        assertTrue(form.canSave)
        // A sheet survives saved state as the editor's two strings.
        assertEquals(form, EditorSheet(sheet.draft, sheet.formJson).form)

        val effect = EditorSheet.decode(fixture("editor-change-effect").getJSONObject("response")).form.effect
        assertEquals("specific", effect.mode)
        assertEquals(listOf("enchantment", "curse"), effect.groups.map { it.value })
        assertEquals(listOf("Blazing"), effect.choices.filter { it.selected }.map { it.value })

        val resin = EditorSheet.decode(fixture("editor-resin-amount-invalid").getJSONObject("response")).form
        assertTrue(resin.resinPicked)
        assertNull(resin.rowKey)
        assertEquals(SheetResin(visible = true, auto = false, amount = null, includeMageWand = true), resin.resin)
        assertEquals(listOf("Enter an amount from 1 to 65535."), resin.errors)
        assertFalse(resin.canSave)
        assertNull(resin.preview)
    }

    /** The fixture's save request, sent again through [RequirementEditor.save]. */
    private fun replaySave(name: String): SheetSave {
        val request = fixture(name).getJSONObject("request")
        return RequirementEditor.save(
            request.getString("draft"),
            RequirementEditor.decodeRows(request.getJSONArray("rows")),
            if (request.isNull("next_key")) null else request.getLong("next_key"),
        )
    }

    @Test fun savesDecodeTheirRowsAndTheQueryResin() {
        val saved = replaySave("editor-save") as SheetSave.Saved
        assertEquals(listOf(1L, 2L, 3L, 4L), saved.rows!!.map { it.key })
        assertEquals(listOf(find("spear"), find("spear")), saved.rows!!.drop(2).map { it.item })
        assertEquals(6, saved.rows!!.last().maximumDepth)
        assertEquals(3L, saved.focus)
        assertEquals(5L, saved.nextKey)
        assertNull(saved.resin)

        val set = replaySave("editor-resin-save-set") as SheetSave.Saved
        assertEquals(listOf(2L), set.rows!!.map { it.key })
        assertEquals(SavedResin.Set(ResinCondition(4, auto = false, ArcaneResinFilter(maximumDepth = 14))), set.resin)
        assertNull(set.focus)

        val clear = replaySave("editor-resin-save-clear") as SheetSave.Saved
        assertEquals(SavedResin.Clear, clear.resin)
        assertEquals(find("wand_frost"), clear.rows!!.last().item)

        val refused = replaySave("editor-save-refused") as SheetSave.Refused
        assertEquals(
            listOf("This trinket is already required. Each trinket appears only once in the deck."),
            refused.sheet.form.errors,
        )
        assertFalse(refused.sheet.form.canSave)
    }

    @Test fun aSheetOpensChangesAndSavesThroughTheEngine() {
        val frost = ItemRequirement(1, find("wand_frost"), 2)
        var sheet = RequirementEditor.open(listOf(frost), offerResin = true)
        assertTrue(sheet.form.adding)
        assertEquals("Any weapon", sheet.form.title)
        val changes = listOf(
            SheetChange.category("ring"), SheetChange.item("ring_might"), SheetChange.count(2),
            SheetChange.countLevels(true), SheetChange.total(5),
        )
        for (change in changes) sheet = RequirementEditor.change(sheet.draft, change)
        assertEquals("≥ 5 across up to 2", sheet.form.stack.countLevels.valueLabel)
        val saved = RequirementEditor.save(sheet.draft, listOf(frost), nextKey = 10) as SheetSave.Saved
        val rings = saved.rows!!.drop(1)
        assertEquals(listOf(10L, 11L), rings.map { it.key })
        assertEquals(listOf(LevelSum(1, 5), LevelSum(1, 5)), rings.map { it.levelSum })
        assertEquals(10L, saved.focus)
        assertEquals(12L, saved.nextKey)

        // Reopened and saved as it is, the stack changes nothing, so a refine can resume.
        val reopened = RequirementEditor.open(saved.rows!!, key = 10)
        assertEquals(2, reopened.form.stack.count)
        assertTrue(reopened.form.stack.countLevels.enabled)
        val unchanged = RequirementEditor.save(reopened.draft, saved.rows!!) as SheetSave.Saved
        assertNull(unchanged.rows)
        assertEquals(10L, unchanged.focus)
    }

    @Test fun aStackedClusterMemberTurnedTrinketCannotBeSaved() {
        val spear = ItemRequirement(0, find("spear"), 2)
        val stacked = RequirementEditor.board(
            emptyList(), listOf(BoardEdit.Save(null, spear, count = 2, total = null, copyDepth = null)), nextKey = 1,
        ).rows!! + ItemRequirement(3, find("mace"), 2)
        val cluster = RequirementEditor.board(stacked, listOf(BoardEdit.Join(source = 3, target = 1))).rows!!
        val opened = RequirementEditor.open(cluster, key = 3)
        assertTrue(opened.form.inCluster)
        assertFalse(opened.form.stack.visible)

        val trinket = RequirementEditor.change(opened.draft, SheetChange.category("trinket"))
        assertEquals(listOf("Copies can only be grouped with the same item type."), trinket.form.errors)
        assertFalse(trinket.form.canSave)
        val refused = RequirementEditor.save(trinket.draft, cluster) as SheetSave.Refused
        assertEquals(trinket.form.errors, refused.sheet.form.errors)
    }

    @Test fun everyItemTheSheetOffersHasATile() {
        val sheet = RequirementEditor.open(emptyList(), offerResin = true)
        for (family in sheet.form.category.options.map { it.value }) {
            val options = RequirementEditor.change(sheet.draft, SheetChange.category(family)).form.item.options
            val items = options.mapNotNull { it.value }.filter { it != SheetChange.ARCANE_RESIN }
            assertTrue(family, items.isNotEmpty())
            assertEquals(family, emptyList<String>(), items.filter { ItemCatalog.findById(it) == null })
        }
    }

    @Test fun everyChangeIsReadByTheEditor() {
        val sheet = RequirementEditor.open(listOf(ItemRequirement(1, find("wand_frost"), 2)), key = 1, offerResin = true)
        val changes = listOf(
            SheetChange.category("weapon"), SheetChange.weaponType("melee"), SheetChange.item(null),
            SheetChange.item("spear"), SheetChange.tierMode("at_least"), SheetChange.tier(4),
            SheetChange.upgradeMode("at_least"), SheetChange.upgrade(2), SheetChange.effectMode("specific"),
            SheetChange.toggleEffect("Blazing"), SheetChange.uncursed(true), SheetChange.source("locked_chest"),
            SheetChange.source(null), SheetChange.floorLimitEnabled(true), SheetChange.floorLimit(6),
            SheetChange.excludeResin(true), SheetChange.transmutationsEnabled(true), SheetChange.transmutations(3),
            SheetChange.selectTrinket(true), SheetChange.count(3), SheetChange.copyDepthEnabled(true),
            SheetChange.copyDepth(9), SheetChange.countLevels(true), SheetChange.total(4),
            SheetChange.item(SheetChange.ARCANE_RESIN), SheetChange.resinAuto(true), SheetChange.resinAmount(4.0),
            SheetChange.resinAmount(null), SheetChange.resinAmount(Double.NaN), SheetChange.includeMageWand(true),
        )
        for (change in changes) RequirementEditor.change(sheet.draft, change)
        // Floor 5 is an empty boss floor: a step up from floor 4 lands on 6.
        val floored = listOf(SheetChange.floorLimitEnabled(true), SheetChange.floorLimit(5))
            .fold(sheet) { open, change -> RequirementEditor.change(open.draft, change) }
        assertEquals(6, floored.form.floorLimit.value)
        val failure = assertThrows(IllegalStateException::class.java) {
            RequirementEditor.change("{\"v\":99}", SheetChange.uncursed(true))
        }
        assertTrue(failure.message!!.startsWith("The draft cannot be read"))
    }

    @Test fun theBoardOfAnEditIsKeptForTheListItProduced() {
        val views = BoardViews()
        val rows = listOf(ItemRequirement(1, find("wand_frost"), 2))
        val board = views.of(rows, null)
        assertTrue(board === views.of(rows.toList(), null))
        val resin = ResinCondition.of(0, auto = true, ArcaneResinFilter())
        assertEquals("Arcane Resin", views.of(rows, resin).resin!!.name)
        assertNull(ResinCondition.of(0, auto = false, ArcaneResinFilter()))
    }
}
