// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import org.json.JSONArray
import org.json.JSONObject

/**
 * An open requirement sheet: the editor's [draft], which the app keeps and
 * sends back untouched, and the [form] it shows ([RequirementEditor.open]).
 * [formJson] is the form as the editor wrote it, so a sheet can be kept in
 * saved state as two strings.
 */
class EditorSheet(val draft: String, internal val formJson: String) {
    val form: SheetForm = SheetForm.decode(JSONObject(formJson))

    companion object {
        internal fun decode(answer: JSONObject) =
            EditorSheet(answer.getString("draft"), answer.getJSONObject("form").toString())
    }
}

/**
 * Everything the requirement sheet shows for a draft, as the shared core
 * decides and words it (`docs/requirement-editor.md`, "FORM"): which
 * controls show, what they offer, their ranges, labels and help texts, the
 * chip a save would produce, and why the draft cannot be saved yet. The sheet
 * lays these out; its titles, buttons, picker headings and slider names are
 * its own.
 */
data class SheetForm(
    /**
     * Whether the sheet adds rather than edits: a new chip, or the query's
     * Arcane Resin when it asks for none yet.
     */
    val adding: Boolean,
    /** The visible row the sheet edits; null for a new chip or the resin chip. */
    val rowKey: Long?,
    val blanket: Boolean,
    /** The row is an either/or alternative, with a stack of its own. */
    val inCluster: Boolean,
    /** Arcane Resin is the picked item: the sheet edits the query's resin. */
    val resinPicked: Boolean,
    /** What the sheet is about: the requirement's title, or `Arcane Resin`. */
    val title: String,
    /** The kind the sprite stands for while no item is named. */
    val kind: ItemKind?,
    /** The chip a save would put on the board; null while the draft has errors. */
    val preview: ChipView?,
    /** The six families. */
    val category: SheetPicker<String>,
    /** Any, melee or thrown, on weapons. */
    val weaponType: SheetPicker<String>,
    /** The wildcard, Arcane Resin when offered, then the items; null is the wildcard. */
    val item: SheetPicker<String?>,
    val tier: SheetRange,
    val upgrade: SheetRange,
    val effect: SheetEffects,
    val uncursed: SheetToggle,
    /** Null is any source. */
    val source: SheetPicker<String?>,
    val floorLimit: SheetFloors,
    val excludeResin: SheetToggle,
    val transmutations: SheetStepper,
    val selectTrinket: SheetToggle,
    val stack: SheetStack,
    val resin: SheetResin,
    /** Why the draft cannot be saved, in the order to show them. */
    val errors: List<String>,
    val canSave: Boolean,
) {
    companion object {
        internal fun decode(form: JSONObject): SheetForm {
            val origin = form.getJSONObject("origin")
            val kind = form.getJSONObject("kind").getString("value")
            return SheetForm(
                adding = form.getString("mode") == "new",
                rowKey = if (origin.getString("type") == "row") origin.getLong("key") else null,
                blanket = form.getBoolean("blanket"),
                inCluster = form.getBoolean("in_cluster"),
                resinPicked = form.getBoolean("resin_picked"),
                title = form.getString("title"),
                kind = ItemKind.entries.firstOrNull { it.name.lowercase() == kind },
                preview = form.objectOrNull("preview")?.let(BoardView::decodeChip),
                category = form.getJSONObject("category").picker(JSONObject::getString),
                weaponType = form.getJSONObject("weapon_type").picker(JSONObject::getString),
                item = form.getJSONObject("item").picker(JSONObject::stringOrNull),
                tier = form.getJSONObject("tier").range(),
                upgrade = form.getJSONObject("upgrade").range(),
                effect = form.getJSONObject("effect").let { effect ->
                    SheetEffects(
                        visible = effect.getBoolean("visible"),
                        label = effect.getString("label"),
                        mode = effect.getString("mode"),
                        modes = effect.getJSONArray("modes").options(JSONObject::getString),
                        choicesVisible = effect.getBoolean("choices_visible"),
                        choices = effect.getJSONArray("choices").objects().map {
                            SheetEffect(it.getString("value"), it.getString("label"), it.getString("group"), it.getBoolean("selected"))
                        },
                        groups = effect.getJSONArray("groups").options(JSONObject::getString),
                        caption = effect.getString("caption"),
                    )
                },
                uncursed = form.getJSONObject("uncursed").toggle(),
                source = form.getJSONObject("source").picker(JSONObject::stringOrNull),
                floorLimit = form.getJSONObject("floor_limit").floors(),
                excludeResin = form.getJSONObject("exclude_resin").toggle(),
                transmutations = form.getJSONObject("transmutations").stepper(),
                selectTrinket = form.getJSONObject("select_trinket").toggle(),
                stack = form.getJSONObject("stack").let { stack ->
                    SheetStack(
                        visible = stack.getBoolean("visible"),
                        label = stack.getString("label"),
                        count = stack.getInt("count"),
                        min = stack.getInt("min"),
                        max = stack.getInt("max"),
                        valueLabel = stack.getString("value_label"),
                        copyDepth = stack.getJSONObject("copy_depth").floors(),
                        countLevels = stack.getJSONObject("count_levels").stepper(),
                    )
                },
                resin = form.getJSONObject("resin").let { resin ->
                    SheetResin(
                        visible = resin.getBoolean("visible"),
                        label = resin.getString("label"),
                        auto = resin.getBoolean("auto"),
                        modes = resin.getJSONArray("modes").options(JSONObject::getBoolean),
                        caption = resin.getString("caption"),
                        amount = if (resin.isNull("amount")) null else resin.getDouble("amount"),
                        min = resin.getInt("min"),
                        max = resin.getInt("max"),
                        includeMageWand = resin.getJSONObject("include_mage_wand").toggle(),
                    )
                },
                errors = form.getJSONArray("errors").let { errors -> List(errors.length(), errors::getString) },
                canSave = form.getBoolean("can_save"),
            )
        }
    }
}

/** One choice of a picker. [hidden] marks one offered only because the draft already names it. */
data class SheetOption<T>(val value: T, val label: String, val group: String?, val hidden: Boolean)

/** A picker: its value and its choices. */
data class SheetPicker<T>(val visible: Boolean, val value: T, val options: List<SheetOption<T>>) {
    /** The chosen option's label. */
    val label: String? get() = options.firstOrNull { it.value == value }?.label
}

/** A check box or a switch, with the help text shown under it whenever it shows; null for none. */
data class SheetToggle(val visible: Boolean, val value: Boolean, val label: String, val caption: String? = null)

/** A mode picker with a value slider (tier, upgrade); [value] stays within [min]..[max] even while hidden. */
data class SheetRange(
    val visible: Boolean,
    val mode: String,
    val modes: List<SheetOption<String>>,
    /** Whether the value slider shows: the control does, in a mode that takes a value. */
    val valueVisible: Boolean,
    val value: Int,
    val min: Int,
    val max: Int,
    /** The value in words: `Tier 3 or higher`, `+2`. */
    val valueLabel: String,
)

/** The effect filter of a weapon or armor, with the "Specific…" grid's [choices] under their [groups]. */
data class SheetEffects(
    val visible: Boolean,
    /** The section's label: `Enchantment`, or `Glyph` on armor. */
    val label: String,
    val mode: String,
    val modes: List<SheetOption<String>>,
    /** Whether the "Specific…" grid and its [caption] show. */
    val choicesVisible: Boolean,
    val choices: List<SheetEffect>,
    val groups: List<SheetOption<String>>,
    /** What the ticked effects mean. */
    val caption: String,
)

/** One effect of the "Specific…" grid, under the heading its [group] names. */
data class SheetEffect(val value: String, val label: String, val group: String, val selected: Boolean)

/** A switch with a floor slider, whose [options] skip the empty boss floors. */
data class SheetFloors(
    val visible: Boolean,
    val enabled: Boolean,
    val value: Int,
    val options: List<SheetOption<Int>>,
    val label: String,
    /** `Within first 4 floors`. */
    val valueLabel: String,
)

/**
 * A switch with a stepper or slider (transmutations, a combined level), which
 * shows while [enabled]; its [caption] shows while [captionVisible].
 */
data class SheetStepper(
    val visible: Boolean,
    val enabled: Boolean,
    val value: Int,
    val min: Int,
    val max: Int,
    val label: String,
    val caption: String?,
    val captionVisible: Boolean,
    val valueLabel: String,
)

/** How many items the chip asks for, its copies' floor limit, and the level they reach together. */
data class SheetStack(
    val visible: Boolean,
    /** The section's label, which its count stepper goes by: `Total item count`. */
    val label: String,
    val count: Int,
    val min: Int,
    val max: Int,
    /** `×2`. */
    val valueLabel: String,
    val copyDepth: SheetFloors,
    val countLevels: SheetStepper,
)

/**
 * The Arcane Resin section, under [label] (`Minimum resin`), which the amount
 * field goes by too. [modes] is the Amount/Auto choice, valued as [auto] is;
 * [caption] says what Auto means, in the amount field's place while [auto]
 * is on. [amount] is the number as typed, null for an empty field, and
 * [min]..[max] the amounts that save.
 */
data class SheetResin(
    val visible: Boolean,
    val label: String,
    val auto: Boolean,
    val modes: List<SheetOption<Boolean>>,
    val caption: String,
    val amount: Double?,
    val min: Int,
    val max: Int,
    val includeMageWand: SheetToggle,
)

/**
 * One control the user moved (`docs/requirement-editor.md`, "CHANGE"). The
 * values are the ones the form's options carry; a change to a control the
 * form hides changes nothing.
 */
class SheetChange private constructor(private val type: String, private val value: Any?) {
    internal fun json(): JSONObject = JSONObject().put("type", type).put("value", value ?: JSONObject.NULL)

    override fun toString() = "$type($value)"

    companion object {
        /** The item option that turns the sheet to the query's Arcane Resin. */
        const val ARCANE_RESIN = "arcane_resin"

        fun category(family: String) = SheetChange("set_category", family)
        fun weaponType(type: String) = SheetChange("set_weapon_type", type)
        fun item(item: String?) = SheetChange("set_item", item)
        fun tierMode(mode: String) = SheetChange("set_tier_mode", mode)
        fun tier(tier: Int) = SheetChange("set_tier", tier)
        fun upgradeMode(mode: String) = SheetChange("set_upgrade_mode", mode)
        fun upgrade(upgrade: Int) = SheetChange("set_upgrade", upgrade)
        fun effectMode(mode: String) = SheetChange("set_effect_mode", mode)
        fun toggleEffect(effect: String) = SheetChange("toggle_effect", effect)
        fun uncursed(on: Boolean) = SheetChange("set_uncursed", on)
        fun source(source: String?) = SheetChange("set_source", source)
        fun floorLimitEnabled(on: Boolean) = SheetChange("set_floor_limit_enabled", on)
        fun floorLimit(floor: Int) = SheetChange("set_floor_limit", floor)
        fun excludeResin(on: Boolean) = SheetChange("set_exclude_resin", on)
        fun transmutationsEnabled(on: Boolean) = SheetChange("set_transmutations_enabled", on)
        fun transmutations(count: Int) = SheetChange("set_transmutations", count)
        fun selectTrinket(on: Boolean) = SheetChange("set_select_trinket", on)
        fun count(count: Int) = SheetChange("set_count", count)
        fun copyDepthEnabled(on: Boolean) = SheetChange("set_copy_depth_enabled", on)
        fun copyDepth(floor: Int) = SheetChange("set_copy_depth", floor)
        fun countLevels(on: Boolean) = SheetChange("set_count_levels", on)
        fun total(total: Int) = SheetChange("set_total", total)
        fun resinAuto(on: Boolean) = SheetChange("set_resin_auto", on)

        /** The amount as typed; null for a field that holds no number. */
        fun resinAmount(amount: Double?) = SheetChange("set_resin_amount", amount?.takeIf { it.isFinite() })
        fun includeMageWand(on: Boolean) = SheetChange("set_include_mage_wand", on)
    }
}

/** What saving a sheet came to ([RequirementEditor.save]). */
sealed interface SheetSave {
    /**
     * Stored. [rows] is the list to adopt, null when the save changed
     * nothing; [focus] is the chip the save landed in, and [resin] what
     * becomes of the query's Arcane Resin, null when nothing does.
     */
    data class Saved(
        val rows: List<ItemRequirement>?,
        val nextKey: Long,
        val rekeyed: Map<Long, Long>,
        val focus: Long?,
        val resin: SavedResin?,
    ) : SheetSave

    /** The draft cannot be saved; [sheet]'s form says why. */
    data class Refused(val sheet: EditorSheet) : SheetSave
}

/** What a save does to the query's Arcane Resin. */
sealed interface SavedResin {
    /** Arcane Resin was the picked item: the query asks for [condition]. */
    data class Set(val condition: ResinCondition) : SavedResin

    /** The resin chip was saved as a requirement: the query asks for no resin. */
    data object Clear : SavedResin
}

private fun <T> JSONObject.picker(value: JSONObject.(String) -> T) = SheetPicker(
    visible = getBoolean("visible"),
    value = value("value"),
    options = getJSONArray("options").options(value),
)

private fun <T> JSONArray.options(value: JSONObject.(String) -> T) = objects().map {
    SheetOption(it.value("value"), it.getString("label"), it.stringOrNull("group"), it.getBoolean("hidden"))
}

private fun JSONObject.toggle() = SheetToggle(getBoolean("visible"), getBoolean("value"), getString("label"), stringOrNull("caption"))

private fun JSONObject.range() = SheetRange(
    visible = getBoolean("visible"),
    mode = getString("mode"),
    modes = getJSONArray("modes").options(JSONObject::getString),
    valueVisible = getBoolean("value_visible"),
    value = getInt("value"),
    min = getInt("min"),
    max = getInt("max"),
    valueLabel = getString("value_label"),
)

private fun JSONObject.floors() = SheetFloors(
    visible = getBoolean("visible"),
    enabled = getBoolean("enabled"),
    value = getInt("value"),
    options = getJSONArray("options").options(JSONObject::getInt),
    label = getString("label"),
    valueLabel = getString("value_label"),
)

private fun JSONObject.stepper() = SheetStepper(
    visible = getBoolean("visible"),
    enabled = getBoolean("enabled"),
    value = getInt("value"),
    min = getInt("min"),
    max = getInt("max"),
    label = getString("label"),
    caption = stringOrNull("caption"),
    captionVisible = getBoolean("caption_visible"),
    valueLabel = getString("value_label"),
)
