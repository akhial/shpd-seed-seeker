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
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement
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

    private fun show() {
        val atlas = compose.activity.assets.open("third_party/shattered-pixel-dungeon/items.png")
            .use(BitmapFactory::decodeStream)!!.asImageBitmap()
        compose.setContent {
            SeedSeekerTheme {
                CompositionLocalProvider(LocalItemAtlas provides atlas) {
                    Surface {
                        RequirementBoard(
                            requirements = requirements.value, enabled = enabled.value, compact = compact.value,
                            onChange = { requirements.value = it }, onEdit = { _, _ -> },
                            onRemove = { item -> requirements.value = requirements.value.filterIndexed { index, _ -> index !in item.members } },
                            onAdd = {}, arcaneResin = amount.value, arcaneResinAuto = automatic.value, arcaneResinFilter = ArcaneResinFilter(),
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
        val target = node.fetchSemanticsNode().boundsInRoot.center
        compose.onRoot().performTouchInput { moveTo(target, delayMillis = 100); up() }
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
