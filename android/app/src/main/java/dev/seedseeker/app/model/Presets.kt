// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.catalog.ItemCatalog
import java.util.UUID

data class PresetQuery(
    val requirements: List<ItemRequirement>,
    val maximumDepth: Int = 24,
    val requireBlacksmith: Boolean = false,
    val excludeBlacksmithRewards: Boolean = false,
    val wandmakerQuest: WandmakerQuest? = null,
    val challenges: Int = 0,
    val autoApplyTrinket: Boolean = true,
    val arcaneResin: Int = 0,
    val arcaneResinFilter: ArcaneResinFilter = ArcaneResinFilter(),
    val arcaneResinAuto: Boolean = false,
    val floorRequirements: List<FloorRequirement> = emptyList(),
)

data class QueryPreset(
    val id: String = UUID.randomUUID().toString(),
    val name: String,
    val query: PresetQuery,
    val isBuiltIn: Boolean = false,
)

object BuiltInPresets {
    private fun item(id: String) = checkNotNull(ItemCatalog.findById(id)) { "Built-in preset names unknown item $id" }

    val disintegrate = QueryPreset(
        id = "disintegrate",
        name = "DISINTEGRATE",
        isBuiltIn = true,
        query = PresetQuery(
            requirements = listOf(
                ItemRequirement(1, item("wand_disintegration"), 3, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.AT_LEAST),
                ItemRequirement(2, item("wand_disintegration"), 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY),
                ItemRequirement(3, item("wand_disintegration"), 0, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.ANY),
                ItemRequirement(
                    4,
                    item("eye_of_newt"),
                    0,
                    kind = ItemKind.TRINKET,
                    upgradeMatch = UpgradeMatch.ANY,
                    trinketTransmutations = 1,
                ),
                ItemRequirement(5, item("ring_energy"), 2, kind = ItemKind.RING, upgradeMatch = UpgradeMatch.AT_LEAST),
            ),
            maximumDepth = 19,
        ),
    )

    val guerillaAssassin = QueryPreset(
        id = "guerilla-assassin",
        name = "Guerilla Assassin",
        isBuiltIn = true,
        query = PresetQuery(
            requirements = listOf(
                ItemRequirement(
                    1,
                    item("assassins_blade"),
                    3,
                    effect = EffectFilter.named("Blooming"),
                    kind = ItemKind.WEAPON,
                    upgradeMatch = UpgradeMatch.EXACT,
                    maximumDepth = 7,
                ),
                ItemRequirement(
                    2,
                    null,
                    0,
                    effect = EffectFilter.named("Camouflage"),
                    kind = ItemKind.ARMOR,
                    upgradeMatch = UpgradeMatch.ANY,
                ),
                ItemRequirement(3, item("ring_arcana"), 2, kind = ItemKind.RING, upgradeMatch = UpgradeMatch.AT_LEAST),
            ),
        ),
    )

    /** Floor 17 must be a farming floor, the requirement the farming-floor toggle writes. */
    val ringOfWealth = QueryPreset(
        id = "ring-of-wealth",
        name = "Ring of Wealth",
        isBuiltIn = true,
        query = PresetQuery(
            autoApplyTrinket = false,
            requirements = listOf(
                ItemRequirement(1, item("ring_wealth"), 4, kind = ItemKind.RING, upgradeMatch = UpgradeMatch.EXACT),
                ItemRequirement(
                    2,
                    item("dried_rose"),
                    0,
                    kind = ItemKind.ARTIFACT,
                    upgradeMatch = UpgradeMatch.ANY,
                    maximumDepth = 9,
                ),
                ItemRequirement(
                    3,
                    null,
                    3,
                    kind = ItemKind.ARMOR,
                    tier = 4,
                    tierMatch = TierMatch.AT_MOST,
                    upgradeMatch = UpgradeMatch.EXACT,
                    maximumDepth = 4,
                ),
                ItemRequirement(
                    4,
                    null,
                    3,
                    kind = ItemKind.WEAPON,
                    tier = 4,
                    tierMatch = TierMatch.AT_MOST,
                    upgradeMatch = UpgradeMatch.EXACT,
                    maximumDepth = 9,
                ),
                ItemRequirement(
                    5,
                    item("dimensional_sundial"),
                    0,
                    kind = ItemKind.TRINKET,
                    upgradeMatch = UpgradeMatch.ANY,
                    trinketTransmutations = 1,
                ),
            ),
            floorRequirements = listOf(
                FloorRequirement(17, FloorFeeling.DARK, anyRooms = listOf("garden", "secret_garden")),
            ),
        ),
    )

    val necromancer = QueryPreset(
        id = "necromancer",
        name = "Necromancer",
        isBuiltIn = true,
        query = PresetQuery(
            requirements = listOf(
                ItemRequirement(1, item("wand_corruption"), 3, kind = ItemKind.WAND, upgradeMatch = UpgradeMatch.EXACT),
                ItemRequirement(
                    2,
                    null,
                    3,
                    kind = ItemKind.WEAPON,
                    tier = 5,
                    tierMatch = TierMatch.EXACT,
                    upgradeMatch = UpgradeMatch.EXACT,
                ),
                ItemRequirement(3, item("plate_armor"), 3, kind = ItemKind.ARMOR, upgradeMatch = UpgradeMatch.EXACT),
            ),
            maximumDepth = 14,
            wandmakerQuest = WandmakerQuest.CORPSE_DUST,
        ),
    )

    val bloodBerserker = QueryPreset(
        id = "blood-berserker",
        name = "Blood Berserker",
        isBuiltIn = true,
        query = PresetQuery(
            requirements = listOf(
                ItemRequirement(
                    1,
                    null,
                    3,
                    effect = EffectFilter.named("Vampiric"),
                    kind = ItemKind.WEAPON,
                    tier = 5,
                    tierMatch = TierMatch.EXACT,
                    upgradeMatch = UpgradeMatch.EXACT,
                ),
                ItemRequirement(
                    2,
                    item("plate_armor"),
                    3,
                    effect = EffectFilter.named("Thorns"),
                    kind = ItemKind.ARMOR,
                    upgradeMatch = UpgradeMatch.EXACT,
                ),
                ItemRequirement(3, item("ring_arcana"), 4, kind = ItemKind.RING, upgradeMatch = UpgradeMatch.EXACT),
                ItemRequirement(4, item("chalice_of_blood"), 0, kind = ItemKind.ARTIFACT, upgradeMatch = UpgradeMatch.ANY),
            ),
        ),
    )

    val all = listOf(disintegrate, guerillaAssassin, ringOfWealth, necromancer, bloodBerserker)
}
