package dev.seedseeker.app.ui

import androidx.activity.ComponentActivity
import androidx.compose.runtime.*
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import dev.seedseeker.app.model.*
import dev.seedseeker.app.ui.theme.SeedSeekerTheme
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(ScoutRobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "w360dp-h800dp-xhdpi")
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class FarmingFloorsUiTest {
    @get:Rule val compose = createAndroidComposeRule<ComponentActivity>()

    @Test fun segmentedControlSelectsIndependentFloorsAndExplainsTheRequirement() {
        var query by mutableStateOf(PresetQuery(emptyList(), maximumDepth = 4))
        compose.setContent {
            SeedSeekerTheme {
                FarmingFloorsSection(query.floorRequirements, true,
                    onToggle = { query = query.toggleFarmingFloor(it) }, onRemove = {})
            }
        }
        compose.onNodeWithContentDescription("Floor 7").performClick().assertIsOn()
        compose.onNodeWithContentDescription("Floor 22").performClick().assertIsOn()
        compose.onNodeWithContentDescription("Floor 7").performClick().assertIsOff()
        compose.runOnIdle {
            assertEquals(22, query.maximumDepth)
            assertEquals(listOf(22), query.floorRequirements.map { it.depth })
            assertTrue(query.requirements.isEmpty())
        }
        compose.onNodeWithText("Dark floor with a garden.").assertIsDisplayed()
    }
}
