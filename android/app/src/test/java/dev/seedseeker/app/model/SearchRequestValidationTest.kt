// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.catalog.PackagedCatalog
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

/**
 * The query-level checks and model invariants the app keeps itself. What is
 * wrong with the requirement list — stacks, combined levels, either/or
 * groups — is the shared requirement editor's (RequirementEditorTest), and
 * the engine refuses the same queries (QueryDocumentTest pins its refusals).
 */
class SearchRequestValidationTest {
    init { PackagedCatalog.install() }

    private val might = ItemCatalog.findById("ring_might")
    private val sword = ItemCatalog.findById("sword")

    /** A tier-4 weapon: the one thing the vault levels past +4. */
    private val battleAxe = ItemCatalog.findById("battle_axe")

    @Test
    fun anEmptyListAsksForARequirement() {
        assertEquals("Add at least one requirement.", emptyList<ItemRequirement>().validationProblem())
        assertThrows(IllegalArgumentException::class.java) { SearchRequest(emptyList()) }
    }

    @Test
    fun slotsGroupAlternativesAtTheirFirstMembersPosition() {
        val rows = listOf(
            ItemRequirement(1, sword, 1, alternativeGroup = 3),
            ItemRequirement(2, might, 1),
            ItemRequirement(3, sword, 2, alternativeGroup = 3),
            ItemRequirement(4, sword, 3, alternativeGroup = 9),
        )
        assertEquals(listOf(listOf(1L, 3L), listOf(2L), listOf(4L)), rows.slots().map { slot -> slot.map { it.key } })
        assertEquals(3, rows.slotCount())
        assertEquals(3, SearchRequest(rows).slotCount)
    }

    @Test
    fun effectFiltersCanonicalizeToTheSharedForm() {
        assertEquals(EffectFilter.Any, EffectFilter.of(emptyList(), ItemKind.WEAPON))
        assertEquals(EffectFilter.OneOf(listOf("Blazing", "Lucky")), EffectFilter.of(setOf("Lucky", "Blazing"), ItemKind.MELEE_WEAPON))
        assertEquals(EffectFilter.AnyEnchantment, EffectFilter.of(ItemCatalog.enchantments.shuffled(), ItemKind.WEAPON))
        assertEquals(
            EffectFilter.OneOf(ItemCatalog.enchantments + listOf("Annoying")),
            EffectFilter.of(ItemCatalog.enchantments + listOf("Annoying"), ItemKind.WEAPON),
        )
        assertEquals("Blazing", ItemRequirement(1, sword, 1, effect = EffectFilter.named("Blazing")).singleEffect)
        assertNull(ItemRequirement(1, sword, 1, effect = EffectFilter.OneOf(listOf("Blazing", "Lucky"))).singleEffect)
        assertThrows(IllegalArgumentException::class.java) { EffectFilter.OneOf(emptyList()) }
        assertThrows(IllegalArgumentException::class.java) { EffectFilter.OneOf(listOf("Lucky", "Lucky")) }
    }

    /** Model invariants the engine also enforces; pinned here since the SSF8 codec tests that held them are gone. */
    @Test
    fun modelInvariantsAreEnforcedLocally() {
        val anyWeapon = ItemRequirement(1, null, 0, kind = ItemKind.WEAPON, upgradeMatch = UpgradeMatch.ANY)
        assertThrows(IllegalArgumentException::class.java) { SearchRequest(listOf(anyWeapon), challenges = 512) }
        // Tier bounds.
        assertEquals(5, anyWeapon.copy(tier = 5, tierMatch = TierMatch.EXACT).tier)
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 1, tierMatch = TierMatch.EXACT) }
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 6, tierMatch = TierMatch.EXACT) }
        assertEquals(4, anyWeapon.copy(tier = 4, tierMatch = TierMatch.AT_MOST).tier)
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 5, tierMatch = TierMatch.AT_MOST) }
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 2, tierMatch = TierMatch.AT_MOST) }
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 2, tierMatch = TierMatch.AT_LEAST) }
        assertThrows(IllegalArgumentException::class.java) { anyWeapon.copy(tier = 5, tierMatch = TierMatch.AT_LEAST) }
        // A narrowed weapon kind rejects the other class.
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(1, sword, 0, kind = ItemKind.THROWN_WEAPON, upgradeMatch = UpgradeMatch.ANY)
        }
        assertEquals("Any melee weapon", anyWeapon.copy(kind = ItemKind.MELEE_WEAPON).title)
        // The vault's +5 lands on a tier-4 weapon and nothing else: another
        // tier, another family, and a tier filter that rules tier 4 out all
        // stop at +4.
        assertEquals(5, ItemRequirement(1, battleAxe, 5).upgrade)
        assertEquals(5, ItemRequirement(1, ItemCatalog.findById("javelin"), 5).upgrade)
        assertEquals(4, ItemRequirement(1, sword, 4).upgrade)
        assertEquals(4, ItemRequirement(1, might, 4).upgrade)
        assertThrows(IllegalArgumentException::class.java) { ItemRequirement(1, battleAxe, 6) }
        assertThrows(IllegalArgumentException::class.java) { ItemRequirement(1, sword, 5) }
        assertThrows(IllegalArgumentException::class.java) { ItemRequirement(1, ItemCatalog.findById("wand_frost"), 5) }
        assertEquals(5, anyWeapon.copy(upgrade = 5, upgradeMatch = UpgradeMatch.EXACT, tier = 4, tierMatch = TierMatch.EXACT).upgrade)
        assertThrows(IllegalArgumentException::class.java) {
            anyWeapon.copy(upgrade = 5, upgradeMatch = UpgradeMatch.EXACT, tier = 5, tierMatch = TierMatch.EXACT)
        }
        assertThrows(IllegalArgumentException::class.java) {
            anyWeapon.copy(upgrade = 5, upgradeMatch = UpgradeMatch.EXACT, tier = 3, tierMatch = TierMatch.AT_MOST)
        }
        // Uncursed with a curses-only set.
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(1, sword, 1, effect = EffectFilter.named("Displacing"), requireUncursed = true)
        }
        assertThrows(IllegalArgumentException::class.java) {
            ItemRequirement(1, sword, 1, effect = EffectFilter.OneOf(listOf("Annoying", "Sacrificial")), requireUncursed = true)
        }
        // Quest names are the exact snake_case document names.
        assertEquals(WandmakerQuest.ELEMENTAL_EMBERS, WandmakerQuest.named("elemental_embers"))
        assertNull(WandmakerQuest.named("elemental embers"))
        assertEquals("Rotberry", WandmakerQuest.ROTBERRY.label)
    }
}
