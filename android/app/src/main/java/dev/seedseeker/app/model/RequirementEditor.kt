// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.model

import dev.seedseeker.app.engine.JniBindings
import org.json.JSONArray
import org.json.JSONObject

/**
 * The requirement board's rules, all of them the shared core's
 * (`crates/seedfinder-core/src/editor/`, specified in
 * `docs/requirement-editor.md`) and reached through [JniBindings]: how the
 * flat requirement list folds into chips, either/or clusters and stacks,
 * what a drop or a save writes back, what every chip says, and what is wrong
 * with the list. What remains here is the mapping between the app's
 * requirement list and the editor's rows, the same boundary convention
 * [ResultsExport] keeps for query documents.
 *
 * A row is one requirement object of the canonical query document, written
 * and read by [ResultsExport], plus the requirement's [ItemRequirement.key]
 * and its [ItemRequirement.alternativeGroup].
 */
object RequirementEditor {
    /**
     * The board of [rows], with [resin]'s chip when the query asks for resin.
     * Never throws: a board the editor cannot answer comes back empty, with
     * the reason as its one problem.
     */
    fun view(rows: List<ItemRequirement>, resin: ResinCondition? = null): BoardView =
        runCatching { board(rows, resin = resin).board }
            .getOrElse { BoardView.unavailable(it.message ?: "The requirement board could not be read.") }

    /**
     * Applies [edits] to [rows] in order; a new row takes its key from
     * [nextKey] on, when given.
     *
     * @throws IllegalStateException when the editor cannot read the request,
     *   or answers with a row this app cannot hold.
     */
    fun board(
        rows: List<ItemRequirement>,
        edits: List<BoardEdit> = emptyList(),
        resin: ResinCondition? = null,
        nextKey: Long? = null,
    ): BoardAnswer {
        val request = JSONObject().apply {
            put("rows", encodeRows(rows))
            nextKey?.let { put("next_key", it) }
            if (edits.isNotEmpty()) put("edits", JSONArray(edits.map(BoardEdit::json)))
            resin?.let { put("resin", it.json()) }
        }
        return BoardAnswer.decode(answer(JniBindings.requirementBoard(request.toString().toByteArray())))
    }

    /**
     * A list just loaded or imported, as the board keeps it: keyed
     * [firstKey], [firstKey] + 1, … in list order, and in the editor's
     * canonical encoding. The keys stay as given should the editor fail.
     */
    fun loaded(requirements: List<ItemRequirement>, firstKey: Long): List<ItemRequirement> {
        val keyed = requirements.mapIndexed { index, requirement -> requirement.copy(key = firstKey + index) }
        return runCatching { board(keyed, listOf(BoardEdit.Normalize)).rows }.getOrNull() ?: keyed
    }

    /** The editor's answer, or its `{"error"}` as an exception. */
    internal fun answer(bytes: ByteArray): JSONObject {
        val answer = JSONObject(String(bytes, Charsets.UTF_8))
        check(!answer.has("error")) { answer.getString("error") }
        return answer
    }

    internal fun encodeRows(rows: List<ItemRequirement>) = JSONArray(rows.map(::encodeRow))

    internal fun encodeRow(requirement: ItemRequirement): JSONObject =
        ResultsExport.encodeRequirement(requirement).apply {
            put("key", requirement.key)
            requirement.alternativeGroup?.let { put("alternative_group", it) }
        }

    /** @throws IllegalArgumentException for a row [ItemRequirement] cannot hold. */
    internal fun decodeRows(rows: JSONArray): List<ItemRequirement> = List(rows.length()) { index ->
        val row = rows.getJSONObject(index)
        ResultsExport.decodeRequirement(row, index, key = row.getLong("key")).copy(
            alternativeGroup = if (row.isNull("alternative_group")) null else row.getInt("alternative_group"),
        )
    }
}

/**
 * One board edit (`docs/requirement-editor.md`, "EDIT"). Keys name visible
 * rows — a chip, or one member of a cluster — never a stack's hidden copies.
 */
sealed interface BoardEdit {
    fun json(): JSONObject

    /** Rewrites the list into its canonical encoding, once when it is loaded. */
    data object Normalize : BoardEdit {
        override fun json() = edit("normalize")
    }

    /** Makes [source] an either/or alternative of [target], any member of a chip or cluster. */
    data class Join(val source: Long, val target: Long) : BoardEdit {
        override fun json() = edit("join").put("source", source).put("target", target)
    }

    /** Takes a cluster member out on its own, leaving the cluster's stack behind. */
    data class Detach(val key: Long) : BoardEdit {
        override fun json() = edit("detach").put("key", key)
    }

    /** Removes a cluster member, or a lone chip's whole entry. */
    data class Remove(val key: Long) : BoardEdit {
        override fun json() = edit("remove").put("key", key)
    }

    /** Removes the whole entry holding [key]: its members and hidden copies. */
    data class RemoveItem(val key: Long) : BoardEdit {
        override fun json() = edit("remove_item").put("key", key)
    }

    /** How many items the entry asks for. */
    data class SetCount(val key: Long, val count: Int) : BoardEdit {
        override fun json() = edit("set_count").put("key", key).put("count", count)
    }

    /** Sets or clears the combined level the stack's items reach together. */
    data class SetTotal(val key: Long, val total: Int?) : BoardEdit {
        override fun json() = edit("set_total").put("key", key).put("total", total ?: JSONObject.NULL)
    }

    /** Turns counting levels together on or off. */
    data class ToggleLevels(val key: Long) : BoardEdit {
        override fun json() = edit("toggle_levels").put("key", key)
    }

    /** Sets or clears the floor limit of the stack's hidden copies. */
    data class SetCopyDepth(val key: Long, val maximumDepth: Int?) : BoardEdit {
        override fun json() = edit("set_copy_depth").put("key", key).put("max_depth", maximumDepth ?: JSONObject.NULL)
    }

    /**
     * Stores [requirement] with its stack's shape onto the row [key], or as
     * a new row at the end when [key] is null. The requirement's own group
     * labels are the editor's to write and are not read.
     */
    data class Save(
        val key: Long?,
        val requirement: ItemRequirement,
        val count: Int,
        val total: Int?,
        val copyDepth: Int?,
    ) : BoardEdit {
        override fun json() = edit("save")
            .put("key", key ?: JSONObject.NULL)
            .put("requirement", ResultsExport.encodeRequirement(requirement))
            .put("count", count)
            .put("total", total ?: JSONObject.NULL)
            .put("copy_depth", copyDepth ?: JSONObject.NULL)
    }
}

private fun edit(type: String) = JSONObject().put("type", type)

/** What the editor answered a board request. */
data class BoardAnswer(
    /** The list after the edits, to adopt; null when they changed nothing. */
    val rows: List<ItemRequirement>?,
    /** The key a new row takes next. */
    val nextKey: Long,
    /** Keys the editor repaired, old to new, for state still holding an old one. */
    val rekeyed: Map<Long, Long>,
    /** The row to follow after the last edit that applied. */
    val focus: Long?,
    /** Why an edit was refused, in the editor's words; the edits before it still applied. */
    val refused: String?,
    /** The board of the list the answer leaves. */
    val board: BoardView,
) {
    companion object {
        internal fun decode(answer: JSONObject): BoardAnswer {
            // The core only writes rows every platform model can hold; one
            // this app cannot is a bug to report, never a list to adopt.
            val rows = if (answer.getBoolean("changed")) {
                runCatching { RequirementEditor.decodeRows(answer.getJSONArray("rows")) }.getOrElse {
                    throw IllegalStateException("The requirement editor answered a row this app cannot hold: ${it.message}", it)
                }
            } else {
                null
            }
            val rekeyed = answer.getJSONArray("rekeyed")
            return BoardAnswer(
                rows = rows,
                nextKey = answer.getLong("next_key"),
                rekeyed = (0 until rekeyed.length()).associate { index ->
                    rekeyed.getJSONArray(index).let { it.getLong(0) to it.getLong(1) }
                },
                focus = if (answer.isNull("focus")) null else answer.getLong("focus"),
                refused = if (answer.isNull("refused")) null else answer.getJSONObject("refused").getString("message"),
                board = BoardView.decode(answer),
            )
        }
    }
}

/** The query's Arcane Resin condition, which the board's resin chip describes. */
data class ResinCondition(val amount: Int, val auto: Boolean, val filter: ArcaneResinFilter) {
    internal fun json(): JSONObject = JSONObject().apply {
        put("amount", if (auto) "auto" else amount)
        put(
            "filter",
            JSONObject().apply {
                put("uncursed", filter.uncursed)
                put("max_depth", filter.maximumDepth ?: JSONObject.NULL)
                put("source", filter.source?.name?.lowercase() ?: JSONObject.NULL)
                put("include_mage_wand", filter.includeMageWand)
            },
        )
    }

    companion object {
        /** The condition of a query asking for [amount] resin, or Auto; null when it asks for none. */
        fun of(amount: Int, auto: Boolean, filter: ArcaneResinFilter): ResinCondition? =
            ResinCondition(amount, auto, filter).takeIf { auto || amount in 1..65535 }
    }
}

/**
 * The board last answered for one list, so the board an edit's answer
 * already carries is drawn rather than asked for again.
 */
class BoardViews {
    private var rows: List<ItemRequirement>? = null
    private var resin: ResinCondition? = null
    private var board: BoardView? = null

    /** The board of [rows] with [resin]'s chip, asked for only when it is not the one kept. */
    fun of(rows: List<ItemRequirement>, resin: ResinCondition?): BoardView {
        board?.takeIf { rows == this.rows && resin == this.resin }?.let { return it }
        return RequirementEditor.view(rows, resin).also { keep(rows, resin, it) }
    }

    /** Keeps [board], an answer already drawn for [rows]. */
    fun keep(rows: List<ItemRequirement>, resin: ResinCondition?, board: BoardView) {
        this.rows = rows
        this.resin = resin
        this.board = board
    }
}
