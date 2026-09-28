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
import dev.seedseeker.app.model.UpgradeMatch
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
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

    @Test fun droppingArmorOnACountedRingKeepsTheRingAndItsCountTogether() {
        val counted = edited(
            emptyList(),
            BoardEdit.Save(null, ItemRequirement(0, ItemCatalog.findById("ring_energy")!!, 4), count = 3, total = null, copyDepth = 20),
        ) + ItemRequirement(9, ItemCatalog.findById("plate_armor")!!, 3)
        requirements.value = counted
        amount.value = 0
        show()

        for (isCompact in listOf(false, true)) {
            compose.runOnIdle { compact.value = isCompact }
            val ringChip = compose.onNodeWithContentDescription("Ring of Energy,", substring = true)
            val armorChip = compose.onNodeWithContentDescription("Plate Armor,", substring = true)
            pickUp(armorChip)
            moveOver(ringChip)
            compose.onNodeWithText("Copies can only be grouped with the same item type.").assertIsDisplayed()
            release()
            compose.onNodeWithText("Copies can only be grouped with the same item type.").assertDoesNotExist()
            compose.runOnIdle { assertEquals(counted, requirements.value) }
            ringChip.assert(hasText("+4"))
            compose.onNode(hasText("×3") and hasAnyAncestor(hasContentDescription("Ring of Energy,", substring = true)))
                .assertIsDisplayed()

            pickUp(ringChip)
            dropOn(armorChip)
            compose.runOnIdle { assertEquals(counted, requirements.value) }
        }
    }

    @Test fun aRejectedDropDoesNotDetachAMemberFromItsOriginalGroup() {
        val counted = edited(
            emptyList(),
            BoardEdit.Save(null, ItemRequirement(0, ItemCatalog.findById("ring_energy")!!, 4), count = 3, total = null, copyDepth = null),
        ) + listOf(original[0].copy(key = 9), original[1].copy(key = 10))
        val grouped = edited(counted, BoardEdit.Join(source = 9, target = 10))
        requirements.value = grouped
        amount.value = 0
        show()

        pickUp(firstWand())
        dropOn(compose.onNodeWithContentDescription("Ring of Energy,", substring = true))
        compose.runOnIdle { assertEquals(grouped, requirements.value) }
        compose.onNodeWithText("or").assertIsDisplayed()
    }

    @Test fun compatibleRingDropKeepsTheCountOnTheEitherOrGroup() {
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
            assertEquals(3, item.count)
            assertEquals(20, item.copyDepth)
            assertEquals(listOf(20, 20), joined.filter { it.key !in item.members }.map { it.maximumDepth })
            assertEquals(emptyList<Any>(), board.problems)
        }
        compose.onNodeWithText("or").assertIsDisplayed()
        compose.onNodeWithText("×3").assertIsDisplayed()
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
        val open = compose.onRoot().fetchSemanticsNode().boundsInRoot.let { Offset(it.center.x, it.bottom - 40f) }
        compose.onRoot().performTouchInput { moveTo(open, delayMillis = 100) }
        release()
        compose.runOnIdle { assertTrue(requirements.value.all { it.alternativeGroup == null }) }
        compose.onNodeWithText("or").assertDoesNotExist()
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

    @Test fun searchingDisablesResinEditingAndDragging() {
        enabled.value = false
        show()
        resin().assertIsNotEnabled().performClick()
        resin().performTouchInput { longClick() }
        compose.onNodeWithText("Drop to remove").assertDoesNotExist()
        compose.runOnIdle { assertEquals(0, edits); assertEquals(0, removals) }
    }
}

internal fun ComposeTestRule.captureResinScreenshot(name: String, window: Window) {
    waitForIdle()
    System.setProperty("robolectric.pixelCopyRenderMode", "hardware")
    val bitmap = Bitmap.createBitmap(window.decorView.width, window.decorView.height, Bitmap.Config.ARGB_8888)
    var copyResult: Int? = null
    runOnIdle { PixelCopy.request(window, bitmap, { copyResult = it }, Handler(Looper.getMainLooper())) }
    waitUntil { copyResult != null }
    assertEquals(PixelCopy.SUCCESS, copyResult)
    val output = File("build/outputs/resin-ui/$name.png")
    output.parentFile?.mkdirs()
    output.outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }
}
