// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.view.PixelCopy
import android.view.Window
import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Surface
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.BoardEdit
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
import dev.seedseeker.app.model.LevelSum
import dev.seedseeker.app.model.RequirementEditor
import dev.seedseeker.app.model.ResinCondition
import dev.seedseeker.app.model.StackView
import dev.seedseeker.app.model.UpgradeMatch
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import dev.seedseeker.app.ui.theme.SpdDanger
import dev.seedseeker.app.ui.theme.SpdGreen
import java.io.File
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w412dp-h915dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class RequirementBoardTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()
    init { PackagedCatalog.install() }

    private val original = listOf(
        ItemRequirement(key = 1, item = null, kind = ItemKind.WAND, upgrade = 3),
        ItemRequirement(key = 2, item = null, kind = ItemKind.WAND, upgrade = 2, maximumDepth = 4),
    )
    private val requirements = mutableStateOf(original)
    private val amount = mutableStateOf(6)
    private val automatic = mutableStateOf(false)
    private val compact = mutableStateOf(false)
    private val enabled = mutableStateOf(true)
    private var edits = 0
    private var removals = 0

    /** A board of rows built through the editor, as the app builds them. */
    private fun edited(rows: List<ItemRequirement>, vararg edits: BoardEdit): List<ItemRequirement> =
        RequirementEditor.board(rows, edits.asList(), nextKey = 1).rows!!

    private fun show() {
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas) {
                    Surface {
                        val resin = ResinCondition.of(amount.value, automatic.value, ArcaneResinFilter())
                        val board = remember(requirements.value, resin) { RequirementEditor.view(requirements.value, resin) }
                        RequirementBoard(
                            board = board, enabled = enabled.value, compact = compact.value,
                            onChange = { edit ->
                                RequirementEditor.board(requirements.value, listOf(edit), resin).rows?.let { requirements.value = it }
                            },
                            onEdit = {}, onAdd = {}, resin = board.resin,
                            onEditResin = { edits++ }, onRemoveResin = { removals++; amount.value = 0; automatic.value = false },
                            modifier = Modifier.width(380.dp).padding(16.dp),
                        )
                    }
                }
            }
        }
    }

    private fun resin() = compose.onNodeWithContentDescription("Arcane Resin,", substring = true)
    private fun firstWand() = compose.onNode(hasContentDescription("Any wand,", substring = true) and hasText("+3"))

    private fun pickUp(node: SemanticsNodeInteraction) {
        val start = node.fetchSemanticsNode().boundsInRoot.center
        compose.onRoot().performTouchInput {
            down(start)
            advanceEventTime(700)
            moveBy(Offset.Zero)
        }
        compose.onNodeWithText("Drop to remove").assertIsDisplayed()
    }

    private fun dropOn(node: SemanticsNodeInteraction) {
        moveOver(node)
        release()
    }

    private fun moveOver(node: SemanticsNodeInteraction) {
        val target = node.fetchSemanticsNode().boundsInRoot.center
        compose.onRoot().performTouchInput { moveTo(target, delayMillis = 100) }
    }

    /**
     * Lets the chip in hand go in the board's top margin, clear of every chip
     * and of the bin, which opens at the board's bottom.
     */
    private fun letGoOnTheOpenBoard() {
        val open = compose.onRoot().fetchSemanticsNode().boundsInRoot.let { Offset(it.center.x, it.top + 4f) }
        compose.onRoot().performTouchInput { moveTo(open, delayMillis = 100) }
        release()
    }

    private fun release() {
        compose.onRoot().performTouchInput { up() }
        compose.onNodeWithText("Drop to remove").assertDoesNotExist()
    }

    @Test fun resinSharesChipSizingAndEditsInBothDisplayModes() {
        show()
        for (isCompact in listOf(false, true)) {
            compose.runOnIdle { compact.value = isCompact }
            val resinBounds = resin().assertIsDisplayed().fetchSemanticsNode().boundsInRoot
            val wandBounds = firstWand().fetchSemanticsNode().boundsInRoot
            assertEquals(wandBounds.height, resinBounds.height, 1f)
            compose.onNodeWithText("≥6", useUnmergedTree = true).assertIsDisplayed()
            resin().performClick()
            compose.captureResinScreenshot(if (isCompact) "board-compact" else "board", compose.activity.window)
        }
        compose.runOnIdle { assertEquals(2, edits) }
    }

    @Test fun autoChipSupportsBothDisplayModesAndRemovalWithZeroFixedAmount() {
        amount.value = 0
        automatic.value = true
        show()
        for (isCompact in listOf(false, true)) {
            compose.runOnIdle { compact.value = isCompact }
            compose.onNodeWithText("Auto", useUnmergedTree = true).assertIsDisplayed()
            resin().performClick()
        }
        compose.captureResinScreenshot("board-auto", compose.activity.window)
        compose.runOnIdle { assertEquals(2, edits) }
        pickUp(resin())
        dropOn(compose.onNodeWithText("Drop to remove"))
        resin().assertDoesNotExist()
    }

    @Test fun aResinTagShowsItsOwnHoverTextToAMouse() {
        amount.value = 0
        automatic.value = true
        show()
        val explanation = "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies"
        val auto = compose.onNodeWithText("Auto", useUnmergedTree = true)
        compose.onNodeWithText(explanation).assertDoesNotExist()
        auto.performMouseInput { enter(center) }
        compose.onNodeWithText(explanation).assertIsDisplayed()
        auto.performMouseInput { exit(center) }
        compose.onNodeWithText(explanation).assertDoesNotExist()
        // A touch keeps the long press for picking the chip up, and shows nothing.
        pickUp(compose.onNodeWithText("Auto", useUnmergedTree = true))
        compose.onNodeWithText(explanation).assertDoesNotExist()
        release()
    }

    @Test fun resinCanBeRemovedByDraggingButCannotJoinAnItemGroup() {
        show()
        pickUp(resin())
        dropOn(firstWand())
        compose.runOnIdle {
            assertEquals(original, requirements.value)
            assertEquals(6, amount.value)
            assertEquals(0, removals)
        }
        pickUp(firstWand())
        dropOn(resin())
        compose.runOnIdle { assertEquals(original, requirements.value) }

        pickUp(resin())
        dropOn(compose.onNodeWithText("Drop to remove"))
        resin().assertDoesNotExist()
        compose.runOnIdle {
            assertEquals(1, removals)
            assertEquals(original, requirements.value)
        }
    }

    @Test fun ordinaryChipsStillJoinAndRemoveWhileResinIsPresent() {
        show()
        pickUp(firstWand())
        dropOn(compose.onAllNodesWithContentDescription("Any wand,", substring = true)[1])
        compose.runOnIdle {
            assertNotNull(requirements.value[0].alternativeGroup)
            assertEquals(requirements.value[0].alternativeGroup, requirements.value[1].alternativeGroup)
        }
        pickUp(firstWand())
        dropOn(compose.onNodeWithText("Drop to remove"))
        compose.runOnIdle {
            assertEquals(listOf(original[1]), requirements.value)
            assertEquals(6, amount.value)
        }
    }

    private val frost = ItemCatalog.findById("wand_frost")!!
    private val disintegration = ItemCatalog.findById("wand_disintegration")!!

    /** The `×N` badge drawn on the chip [name] names, inside it. */
    private fun badgeOn(name: String, text: String) =
        compose.onNode(hasText(text) and hasAnyAncestor(hasContentDescription("$name,", substring = true)))

    /** `{Frost ×2 | Disintegration}`: two Wands of Frost, or one Wand of Disintegration. */
    private val memberStack = listOf(
        ItemRequirement(1, frost, 0, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1, identityGroup = 1),
        ItemRequirement(2, disintegration, 0, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1),
        ItemRequirement(3, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, identityGroup = 1),
    )

    @Test fun droppingArmorOnACountedRingMakesThemAlternativesAndKeepsTheRingsCount() {
        val counted = edited(
            emptyList(),
            BoardEdit.Save(null, ItemRequirement(0, ItemCatalog.findById("ring_energy")!!, 4), count = 3, total = null, copyDepth = 20),
        ) + ItemRequirement(9, ItemCatalog.findById("plate_armor")!!, 3)
        amount.value = 0
        show()

        for (isCompact in listOf(false, true)) {
            compose.runOnIdle {
                compact.value = isCompact
                requirements.value = counted
            }
            val ringChip = compose.onNodeWithContentDescription("Ring of Energy,", substring = true)
            val armorChip = compose.onNodeWithContentDescription("Plate Armor,", substring = true)
            // A stack joins any category now: each of its copies keeps its own kind.
            pickUp(armorChip)
            moveOver(ringChip)
            compose.onNodeWithText("Copies can only be grouped with the same item type.").assertDoesNotExist()
            release()
            compose.onNodeWithText("or").assertIsDisplayed()
            ringChip.assert(hasText("+4"))
            // The ring keeps its ×3 as a member: three rings, or the armor.
            badgeOn("Ring of Energy", "×3").assertIsDisplayed()
            compose.onNode(hasText("×3") and hasAnyAncestor(hasContentDescription("Plate Armor,", substring = true))).assertDoesNotExist()
            compose.runOnIdle {
                val cluster = RequirementEditor.view(requirements.value).items.single()
                assertEquals(listOf(1L, 9L), cluster.members)
                assertEquals(listOf(3, 1), cluster.chips.map { it.stack.count })
            }

            // The other way round, one bare ring moves: the +4 stays with the
            // stack of two left behind, and the ring that joins is the one
            // the chip said a drag lifts.
            compose.runOnIdle { requirements.value = counted }
            val lifted = RequirementEditor.view(counted).itemOf(1)!!.chips.single().lifted!!
            pickUp(ringChip)
            dropOn(armorChip)
            compose.runOnIdle {
                val (rest, cluster) = RequirementEditor.view(requirements.value).items
                assertEquals(2, rest.chips.single().stack.count)
                assertEquals(listOf("+4"), rest.chips.single().tags.map { it.text })
                assertEquals(listOf("Plate Armor" to 1, "Ring of Energy" to 1), cluster.chips.map { it.name to it.stack.count })
                assertEquals(lifted, cluster.chips[1].face)
                assertTrue(cluster.chips[1].tags.none { it.text == "+4" })
            }
        }
    }

    @Test fun aRejectedDropDoesNotDetachAMemberFromItsOriginalGroup() {
        // All four stack labels are in use, so Wand of Frost ×2 cannot keep
        // its stack as a member, and refuses the join.
        val busy = listOf(ItemKind.ARMOR, ItemKind.RING, ItemKind.ARMOR, ItemKind.RING).flatMapIndexed { index, kind ->
            List(2) { copy ->
                ItemRequirement(20L + index * 2 + copy, null, 0, kind = kind, upgradeMatch = UpgradeMatch.ANY, identityGroup = index + 1)
            }
        }
        val grouped = busy + listOf(
            ItemRequirement(9, frost, 0, upgradeMatch = UpgradeMatch.ANY),
            ItemRequirement(10, frost, 0, upgradeMatch = UpgradeMatch.ANY),
        ) + edited(original, BoardEdit.Join(source = 1, target = 2))
        requirements.value = grouped
        amount.value = 0
        show()

        pickUp(firstWand())
        moveOver(compose.onNodeWithContentDescription("Wand of Frost,", substring = true))
        compose.onNodeWithText("Every group label is in use. Remove a stack or a combined level first.").assertIsDisplayed()
        release()
        compose.runOnIdle { assertEquals(grouped, requirements.value) }
        compose.onNodeWithText("or").assertIsDisplayed()
    }

    @Test fun aRingDroppedOnACountedRingLeavesTheCountOnThatRing() {
        val counted = edited(
            emptyList(),
            BoardEdit.Save(null, ItemRequirement(0, ItemCatalog.findById("ring_energy")!!, 4), count = 3, total = null, copyDepth = 20),
        ) + ItemRequirement(9, ItemCatalog.findById("ring_wealth")!!, 2)
        requirements.value = counted
        amount.value = 0
        show()

        pickUp(compose.onNodeWithContentDescription("Ring of Wealth,", substring = true))
        dropOn(compose.onNodeWithContentDescription("Ring of Energy,", substring = true))
        compose.runOnIdle {
            val joined = requirements.value
            val board = RequirementEditor.view(joined)
            val item = board.items.single()
            assertEquals(listOf(1L, 9L), item.members)
            val (energy, wealth) = item.chips
            assertEquals(StackView(count = 3, countMax = 3, total = null, copyDepth = 20), energy.stack)
            assertEquals(1, wealth.stack.count)
            assertEquals(listOf(20, 20), joined.filter { it.key in energy.copies }.map { it.maximumDepth })
            assertEquals(emptyList<Any>(), board.problems)
        }
        compose.onNodeWithText("or").assertIsDisplayed()
        // The badge is the energy ring's, drawn on its chip, and on no other.
        badgeOn("Ring of Energy", "×3").assertIsDisplayed()
        compose.onAllNodesWithText("×3").assertCountEquals(1)
    }

    @Test fun eachMemberWearsItsOwnStackBadge() {
        requirements.value = memberStack
        amount.value = 0
        show()
        badgeOn("Wand of Frost", "×2").assertIsDisplayed()
        compose.onAllNodesWithText("×2").assertCountEquals(1)

        // Members sharing one label are each drawn ×2, and the capsule wears none.
        compose.runOnIdle {
            requirements.value = memberStack.map { if (it.key == 2L) it.copy(identityGroup = 1) else it }
        }
        badgeOn("Wand of Frost", "×2").assertIsDisplayed()
        badgeOn("Wand of Disintegration", "×2").assertIsDisplayed()
        compose.onAllNodesWithText("×2").assertCountEquals(2)
    }

    @Test fun theChipInHandIsOneItemWithoutItsBadges() {
        val chip = RequirementEditor.view(memberStack).itemOf(1)!!.chips.first()
        assertEquals("×2", chip.countBadge?.text)
        compose.setContent { SeedSeekerTheme { Surface { HeldChip(chip) } } }
        compose.onNodeWithContentDescription("Wand of Frost,", substring = true).assertIsDisplayed()
        compose.onNodeWithText("×2", useUnmergedTree = true).assertDoesNotExist()
    }

    /**
     * How many pixels where [node] is drawn are the `×N` badge's green, which
     * no sprite uses. The window is copied whole, since a capture of the node
     * alone waits for a redraw a held drag never gives it.
     */
    private fun badgeGreenPixels(node: SemanticsNodeInteraction): Int {
        val bounds = node.fetchSemanticsNode().boundsInWindow
        val window = compose.captureWindow(compose.activity.window)
        val green = SpdGreen.toArgb()
        val xs = bounds.left.toInt().coerceAtLeast(0) until bounds.right.toInt().coerceAtMost(window.width)
        val ys = bounds.top.toInt().coerceAtLeast(0) until bounds.bottom.toInt().coerceAtMost(window.height)
        return xs.sumOf { x -> ys.count { y -> window.getPixel(x, y) == green } }
    }

    @Test fun aHeldRingLeavesTheRestOfItsStackBehindUntilTheDragIsCancelled() {
        requirements.value = edited(
            emptyList(),
            BoardEdit.Save(null, ItemRequirement(0, ItemCatalog.findById("ring_energy")!!, 4), count = 3, total = null, copyDepth = null),
        )
        amount.value = 0
        show()
        val ringChip = compose.onNodeWithContentDescription("Ring of Energy,", substring = true)
        badgeOn("Ring of Energy", "×3").assertIsDisplayed()
        assertTrue(badgeGreenPixels(ringChip) > 0)

        // One ring is in hand, so its faded place shows the two that stay,
        // with the +4 they keep.
        pickUp(ringChip)
        val origin = hasContentDescription("Ring of Energy, exactly +4")
        compose.onNode(hasText("×2") and hasAnyAncestor(origin)).assertIsDisplayed()
        compose.onNode(origin and hasText("+4")).assertExists()
        compose.onNodeWithText("×3").assertDoesNotExist()
        // The ring in hand is that one ring, without a badge: a bare copy,
        // which asks for no +4.
        assertEquals(0, badgeGreenPixels(compose.onNodeWithTag(HELD_CHIP_TAG)))
        val inHand = hasAnyAncestor(hasTestTag(HELD_CHIP_TAG))
        compose.onNode(inHand and hasContentDescription("Ring of Energy, any upgrade"))
            .assert(hasText("Ring of Energy"))
            .assert(!hasText("+4"))

        // A cancelled drag puts the ring back: the chip reads ×3 again.
        compose.onRoot().performTouchInput { cancel() }
        compose.onNodeWithText("Drop to remove").assertDoesNotExist()
        compose.onNodeWithTag(HELD_CHIP_TAG).assertDoesNotExist()
        badgeOn("Ring of Energy", "×3").assertIsDisplayed()
        compose.onNodeWithText("×2").assertDoesNotExist()
        compose.runOnIdle { assertEquals(3, requirements.value.size) }
    }

    @Test fun theBinTakesOneItemOfAStack() {
        requirements.value = memberStack
        amount.value = 0
        show()
        val frostChip = compose.onNodeWithContentDescription("Wand of Frost,", substring = true)

        // {Frost ×2 | Disintegration} → {Frost | Disintegration}.
        pickUp(frostChip)
        dropOn(compose.onNodeWithText("Drop to remove"))
        compose.onNodeWithText("×2").assertDoesNotExist()
        compose.onNodeWithText("or").assertIsDisplayed()
        compose.runOnIdle { assertEquals(listOf(1L, 2L), requirements.value.map { it.key }) }

        // A member of one leaves, and the cluster of one is a lone chip.
        pickUp(frostChip)
        dropOn(compose.onNodeWithText("Drop to remove"))
        compose.onNodeWithText("or").assertDoesNotExist()
        compose.runOnIdle { assertEquals(listOf(disintegration), requirements.value.map { it.item }) }

        // A lone stack sheds one item at a time.
        compose.runOnIdle { requirements.value = edited(emptyList(), BoardEdit.Save(null, ItemRequirement(0, frost, 2), 2, null, null)) }
        badgeOn("Wand of Frost", "×2").assertIsDisplayed()
        pickUp(frostChip)
        dropOn(compose.onNodeWithText("Drop to remove"))
        compose.onNodeWithText("×2").assertDoesNotExist()
        frostChip.assertIsDisplayed()
        pickUp(frostChip)
        dropOn(compose.onNodeWithText("Drop to remove"))
        frostChip.assertDoesNotExist()
    }

    @Test fun aMemberLeavesItsCapsuleOnlyWhenLetGoOnTheOpenBoard() {
        requirements.value = edited(original, BoardEdit.Join(source = 1, target = 2))
        amount.value = 0
        show()
        val grouped = requirements.value
        // Its own capsule, the other member included, takes no drop.
        pickUp(firstWand())
        dropOn(compose.onNode(hasContentDescription("Any wand,", substring = true) and hasText("+2")))
        compose.runOnIdle { assertEquals(grouped, requirements.value) }
        compose.onNodeWithText("or").assertIsDisplayed()

        pickUp(firstWand())
        letGoOnTheOpenBoard()
        compose.runOnIdle {
            assertTrue(requirements.value.all { it.alternativeGroup == null })
            assertEquals(2, requirements.value.size)
        }
        compose.onNodeWithText("or").assertDoesNotExist()
    }

    @Test fun aMemberLetGoOnTheOpenBoardTakesOneItemOut() {
        requirements.value = memberStack
        amount.value = 0
        show()
        pickUp(compose.onNodeWithContentDescription("Wand of Frost,", substring = true))
        letGoOnTheOpenBoard()
        // {Frost ×2 | Disintegration} → {Frost | Disintegration} + Frost.
        compose.runOnIdle {
            val (cluster, lone) = RequirementEditor.view(requirements.value).items
            assertEquals(listOf("Wand of Frost" to 1, "Wand of Disintegration" to 1), cluster.chips.map { it.name to it.stack.count })
            assertEquals("Wand of Frost" to 1, lone.chips.single().let { it.name to it.stack.count })
        }
        compose.onNodeWithText("×2").assertDoesNotExist()
    }

    @Test fun aMemberLetGoOnTheOpenBoardKeepsItsUpgradeAndSendsOutABareCopy() {
        // {Frost +2 ×2 | Disintegration}: the Frost in hand is a bare copy.
        requirements.value = memberStack.map { if (it.key == 1L) it.copy(upgrade = 2, upgradeMatch = UpgradeMatch.EXACT) else it }
        amount.value = 0
        show()
        val lifted = RequirementEditor.view(requirements.value).itemOf(1)!!.chips.first().lifted!!
        assertEquals("Wand of Frost, any upgrade", lifted.description)
        pickUp(compose.onNodeWithContentDescription("Wand of Frost, exactly +2", substring = true))
        compose.onNode(hasAnyAncestor(hasTestTag(HELD_CHIP_TAG)) and hasContentDescription("Wand of Frost, any upgrade"))
            .assert(!hasText("+2"))
        letGoOnTheOpenBoard()
        // → {Frost +2 | Disintegration} + Wand of Frost.
        compose.runOnIdle {
            val (cluster, lone) = RequirementEditor.view(requirements.value).items
            assertEquals(
                listOf("Wand of Frost" to listOf("+2"), "Wand of Disintegration" to emptyList()),
                cluster.chips.map { chip -> chip.name to chip.tags.map { it.text } },
            )
            assertEquals(lifted, lone.chips.single().face)
        }
        compose.onNodeWithText("×2").assertDoesNotExist()
        compose.onNodeWithText("or").assertIsDisplayed()
    }

    @Test fun aChipTheEditorFindsAProblemWithSaysWhatItIs() {
        val might = ItemCatalog.findById("ring_might")!!
        requirements.value = listOf(
            ItemRequirement(1, might, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 2)),
            ItemRequirement(2, might, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
        )
        amount.value = 0
        show()
        compose.onNodeWithContentDescription("Ring of Might,", substring = true)
            .assert(hasContentDescription("A stack must share one combined level.", substring = true))
    }

    /** How many pixels where [node] is drawn are, within rounding, the error outline's colour. */
    private fun errorPixels(node: SemanticsNodeInteraction): Int {
        val bounds = node.fetchSemanticsNode().boundsInWindow
        val window = compose.captureWindow(compose.activity.window)
        val error = SpdDanger.toArgb()
        fun near(pixel: Int, shift: Int) = kotlin.math.abs((pixel shr shift and 0xFF) - (error shr shift and 0xFF)) <= 8
        val xs = bounds.left.toInt().coerceAtLeast(0) until bounds.right.toInt().coerceAtMost(window.width)
        val ys = bounds.top.toInt().coerceAtLeast(0) until bounds.bottom.toInt().coerceAtMost(window.height)
        return xs.sumOf { x -> ys.count { y -> window.getPixel(x, y).let { near(it, 16) && near(it, 8) && near(it, 0) } } }
    }

    @Test fun aBareCopyInHandLeavesTheProblemOfTheChipItCameFrom() {
        // Ring of Might ×2 whose combined levels disagree, and a wand in an
        // either/or group with a blanket armor: each is outlined in the error
        // colour.
        val might = ItemCatalog.findById("ring_might")!!
        requirements.value = listOf(
            ItemRequirement(1, might, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 3)),
            ItemRequirement(2, null, 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY, alternativeGroup = 1),
            ItemRequirement(3, might, 0, upgradeMatch = UpgradeMatch.ANY, levelSum = LevelSum(1, 4)),
            ItemRequirement(4, null, 0, kind = ItemKind.ARMOR, upgradeMatch = UpgradeMatch.ANY, blanket = true, alternativeGroup = 1),
        )
        amount.value = 0
        show()
        val ring = RequirementEditor.view(requirements.value).itemOf(1)!!.chips.single()
        assertEquals("A stack must share one combined level.", ring.problem)
        assertEquals("Ring of Might, any upgrade", ring.lifted?.description)
        val inHand = hasAnyAncestor(hasTestTag(HELD_CHIP_TAG))

        // The wand has no copies: it moves itself, problem and all.
        pickUp(compose.onNodeWithContentDescription("Any wand,", substring = true))
        compose.onNode(inHand and hasContentDescription("cannot mix ordinary and blanket requirements.", substring = true))
            .assertExists()
        assertTrue(errorPixels(compose.onNodeWithTag(HELD_CHIP_TAG)) > 0)
        compose.onRoot().performTouchInput { cancel() }
        compose.onNodeWithTag(HELD_CHIP_TAG).assertDoesNotExist()

        // The ring stack carries a bare copy, which has no combined level to
        // disagree about: it is drawn and named without the stack's problem.
        val ringChip = compose.onNodeWithContentDescription("Ring of Might,", substring = true)
        assertTrue(errorPixels(ringChip) > 0)
        pickUp(ringChip)
        compose.onNode(inHand and hasContentDescription("Ring of Might, any upgrade")).assertExists()
        compose.onNode(inHand and hasContentDescription("A stack must share one combined level.", substring = true))
            .assertDoesNotExist()
        assertEquals(0, errorPixels(compose.onNodeWithTag(HELD_CHIP_TAG)))
        compose.onRoot().performTouchInput { cancel() }
    }

    @Test fun searchingDisablesResinEditingAndDragging() {
        enabled.value = false
        show()
        resin().assertIsNotEnabled().performClick()
        resin().performTouchInput { longClick() }
        compose.onNodeWithText("Drop to remove").assertDoesNotExist()
        compose.runOnIdle { assertEquals(0, edits); assertEquals(0, removals) }
    }
}

/** What [window] shows once the UI is idle. */
internal fun ComposeTestRule.captureWindow(window: Window): Bitmap {
    waitForIdle()
    System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
    val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
    var copyResult: Int? = null
    runOnIdle { PixelCopy.request(window, bitmap, { copyResult = it }, Handler(Looper.getMainLooper())) }
    waitUntil { copyResult != null }
    assertEquals(PixelCopy.SUCCESS, copyResult)
    return bitmap
}

internal fun ComposeTestRule.captureResinScreenshot(name: String, window: Window) {
    val bitmap = captureWindow(window)
    val output = File("build/outputs/resin-ui/$name.png")
    output.parentFile?.mkdirs()
    output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
}
