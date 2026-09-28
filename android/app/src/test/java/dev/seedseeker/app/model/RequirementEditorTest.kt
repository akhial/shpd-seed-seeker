// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.engine.JniBindings
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
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
