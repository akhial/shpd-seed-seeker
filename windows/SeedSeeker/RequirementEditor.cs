using System.Text.Json.Nodes;

namespace SeedSeeker;

// The requirement board as the shared core's editor answers it
// (crates/seedfinder-core/src/editor, reached through
// seedfinder_requirement_board; the format is docs/requirement-editor.md).
// Every rule of the board lives there — how the flat list folds into chips,
// clusters and stacks, what a drop, a badge or a menu writes back, what each
// chip and badge says, and what is wrong with the list — so this file only
// types the envelope for the window, and the window only draws it. Like
// Models.cs it must stay free of Windows App SDK types: SeedSeeker.Tests
// links it to run on any host.

/// <summary>A request the shared requirement editor could not read, with the row at fault when one was.</summary>
public sealed class RequirementEditorException(string message, long? key = null) : Exception(message)
{
    public long? Key { get; } = key;
}

/// <summary>A tiny qualifier beside a chip's name; the upgrade is tinted apart from the rest.</summary>
public sealed record ChipTag(string Text, bool Upgrade = false);

/// <summary>
/// A chip's effect filter: its words (<c>any enchantment</c>, one effect's
/// name, <c>effect: A/B</c>), the accepted effects in catalog order — every
/// enchantment of the family for "any enchantment" — and which cue it takes.
/// The colours are the window's.
/// </summary>
public sealed record ChipEffect(string Label, IReadOnlyList<string> Effects, bool AnyEnchantment, bool CursesOnly);

/// <summary>What a relation line of a chip's detail is about: its cluster, its combined level, or its stack.</summary>
public enum RelationGlyph { Or, Sum, Times }

public sealed record ChipRelation(RelationGlyph Glyph, string Text);

/// <summary>A row the editor refuses a join onto, and the reason to show while a drag hovers it.</summary>
public sealed record JoinRefusal(long Key, string Reason, string Message);

/// <summary>
/// One visible row: a lone chip, or one member of a cluster.
/// </summary>
/// <param name="Name">The short name beside the sprite: the item, or <c>Any melee</c>.</param>
/// <param name="Title">The full title the detail leads with: the item, or <c>Any Tier 3+ melee weapon</c>.</param>
/// <param name="Item">The item's stable id, or null for a wildcard.</param>
/// <param name="Kind">The kind the sprite is drawn for, or null for a row the editor cannot read.</param>
/// <param name="Tags">Qualifiers after the name, before the effect cue.</param>
/// <param name="TrailingTags">Qualifiers after the effect cue (<c>No resin</c>).</param>
/// <param name="Details">What the chip asks of its item, as the parts of one line.</param>
/// <param name="Description">The accessibility label: the title, then the details.</param>
/// <param name="Problem">The row's own first problem, else the first problem between rows that blames it.</param>
/// <param name="CanDetach">Whether "On its own" applies: the chip is a cluster member.</param>
/// <param name="Join">The visible rows this chip may join, in list order.</param>
/// <param name="Refuse">The visible rows a join onto is refused, with the reason.</param>
public sealed record BoardChip(long Key, string Name, string Title, string? Item, ItemKind? Kind,
    IReadOnlyList<ChipTag> Tags, IReadOnlyList<ChipTag> TrailingTags, ChipEffect? Effect, bool Uncursed,
    IReadOnlyList<string> Details, IReadOnlyList<ChipRelation> Relations, string Description, string? Problem,
    bool InCluster, bool CanDetach, IReadOnlyList<long> Join, IReadOnlyList<JoinRefusal> Refuse)
{
    /// <summary>
    /// The chip's hover detail: its title, what it asks of one item, the
    /// relationships it stands in, and the problem it has, one line each.
    /// </summary>
    public string Detail => string.Join("\n", new[] { Title, Details.Count > 0 ? string.Join(" · ", Details) : null }
        .Concat(Relations.Select(relation => $"{relation.Glyph switch { RelationGlyph.Or => "or", RelationGlyph.Sum => "Σ", _ => "×" }} {relation.Text}"))
        .Append(Problem).OfType<string>());
}

/// <summary>
/// What an entry's count and combined-level steppers offer: how many items it
/// asks for, whether that may grow or change, its combined level and the
/// highest one its items can reach, and the floor limit of its hidden copies.
/// </summary>
public sealed record BoardStack(int Count, int Max, bool CanGrow, bool CanChangeCount, int? Total, bool CanCountLevels,
    int LevelCapacity, int DefaultTotal, int? CopyDepth, bool CanSetCopyDepth, string CountText, string TotalText)
{
    /// <summary>The count stepper's upper bound: an entry that cannot grow may only shed copies.</summary>
    public int CountMaximum => CanGrow ? Max : Count;
}

/// <summary>A badge's words at rest: <c>×3</c> or <c>Σ ≥ 5</c>, and what it means.</summary>
public sealed record BoardBadge(string Text, string CompactText, string Tooltip);

/// <summary>
/// One board entry: a chip, or an either/or cluster of chips, with its stack.
/// </summary>
/// <param name="Id">Stable while the entry survives an edit: <c>r17</c> for a chip, <c>c3</c> for a cluster.</param>
/// <param name="Cluster">The alternative group of a cluster of two or more; null for a chip.</param>
/// <param name="Label">A cluster's caption, <c>Any of 2</c>.</param>
/// <param name="Members">The visible rows' keys: one for a chip, every member of a cluster.</param>
/// <param name="Extras">The hidden copies' keys behind the stack badge.</param>
/// <param name="CountBadge">The <c>×N</c> badge, when the entry asks for more than one item.</param>
/// <param name="TotalBadge">The <c>Σ ≥ T</c> badge, when the entry counts levels together.</param>
/// <param name="Problem">The first problem touching any member or hidden copy.</param>
public sealed record BoardEntry(string Id, bool Blanket, int? Cluster, string? Label, IReadOnlyList<long> Members,
    IReadOnlyList<long> Extras, BoardStack Stack, BoardBadge? CountBadge, BoardBadge? TotalBadge,
    IReadOnlyList<BoardChip> Chips, string? Problem);

public enum ProblemScope { Row, Group, List }

/// <summary>One thing the list must fix before it can be searched, and the rows it blames.</summary>
public sealed record BoardProblem(string Message, IReadOnlyList<long> Keys, ProblemScope Scope);

/// <summary>The Arcane Resin chip: <c>Auto</c> or <c>≥N</c>, <c>Mage +2</c>, <c>F≤N</c>, and the donors in words.</summary>
/// <param name="Tooltip">The donors' source, the one filter no tag shows; null for any source.</param>
/// <param name="AmountTooltip">What the first tag, <c>Auto</c>, means; null for a fixed amount.</param>
public sealed record ResinChip(string Name, IReadOnlyList<ChipTag> Tags, bool Uncursed, string? Tooltip, string? AmountTooltip,
    IReadOnlyList<string> Details, string Description)
{
    /// <summary>The chip's hover detail, laid out like a requirement chip's.</summary>
    public string Detail => Details.Count > 0 ? $"{Name}\n{string.Join(" · ", Details)}" : Name;
}

/// <summary>How many entries each board section shows, a cluster or a stack counting once.</summary>
public sealed record BoardCounts(int Ordinary, int Blanket);

/// <summary>An edit the editor refused, and why.</summary>
public sealed record BoardRefusal(string Reason, string Message);

/// <summary>Where a chip is dropped: onto a chip, a cluster's capsule, a section's empty board, or the remove zone.</summary>
public enum DropKind { Chip, Cluster, Board, Remove }

public enum DropEffect { None, Join, Refused, Detach, Remove }

/// <summary>What a drop does, and the refusal to show when the editor refuses it.</summary>
public sealed record BoardDrop(DropEffect Effect, string? Message = null)
{
    public static BoardDrop None { get; } = new(DropEffect.None);
}

/// <summary>Everything the board draws for one requirement list.</summary>
public sealed class BoardView(IReadOnlyList<BoardEntry> entries, BoardCounts counts, IReadOnlyList<BoardProblem> problems, ResinChip? resin)
{
    public static BoardView Empty { get; } = new([], new(0, 0), [], null);

    /// <summary>The entries in list order, both sections together; <see cref="BoardEntry.Blanket"/> says which.</summary>
    public IReadOnlyList<BoardEntry> Entries { get; } = entries;
    public BoardCounts Counts { get; } = counts;
    /// <summary>Every row's own problems in list order, then those between rows, then the list's.</summary>
    public IReadOnlyList<BoardProblem> Problems { get; } = problems;
    /// <summary>The Arcane Resin chip, when the query asks for resin.</summary>
    public ResinChip? Resin { get; } = resin;

    private readonly Dictionary<long, (BoardEntry Entry, BoardChip Chip)> chips =
        entries.SelectMany(entry => entry.Chips.Select(chip => (entry, chip))).ToDictionary(pair => pair.chip.Key);

    /// <summary>The first problem of the list, or null when the board has none.</summary>
    public string? Problem => Problems.Count > 0 ? Problems[0].Message : null;

    /// <summary>The chip of the visible row <paramref name="key"/>; a stack's hidden copies have none.</summary>
    public BoardChip? ChipOf(long key) => chips.TryGetValue(key, out var pair) ? pair.Chip : null;

    /// <summary>The entry the visible row <paramref name="key"/> belongs to.</summary>
    public BoardEntry? EntryOf(long key) => chips.TryGetValue(key, out var pair) ? pair.Entry : null;

    /// <summary>
    /// What dropping the chip keyed <paramref name="source"/> does, decided
    /// from its chip as the editor answered it: onto a chip or a cluster, a
    /// join the editor offers, or the refusal it gives; onto the empty board of
    /// the chip's own section (<paramref name="blanketBoard"/>), a cluster
    /// member comes out on its own while a lone chip stays where it is; onto
    /// the remove zone, a removal. Anything else does nothing.
    /// </summary>
    /// <param name="target">For a chip, its key; for a cluster, any member's.</param>
    public BoardDrop Drop(long source, DropKind kind, long target = 0, bool blanketBoard = false)
    {
        if (!chips.TryGetValue(source, out var dragged)) return BoardDrop.None;
        return kind switch
        {
            DropKind.Chip or DropKind.Cluster when dragged.Chip.Join.Contains(target) => new(DropEffect.Join),
            DropKind.Chip or DropKind.Cluster => dragged.Chip.Refuse.FirstOrDefault(refusal => refusal.Key == target) is { } refusal
                ? new(DropEffect.Refused, refusal.Message) : BoardDrop.None,
            DropKind.Board => dragged.Entry.Blanket == blanketBoard && dragged.Chip.CanDetach ? new(DropEffect.Detach) : BoardDrop.None,
            DropKind.Remove => new(DropEffect.Remove),
            _ => BoardDrop.None,
        };
    }
}

/// <summary>
/// The editor's answer to a board request.
/// </summary>
/// <param name="Rows">The list after the edits, only when it <paramref name="Changed"/>: a list
/// the edits left as it was is never written back, so nothing that compares
/// queries sees a difference that is not there.</param>
/// <param name="Rekeyed">Keys the editor repaired, old to new.</param>
/// <param name="Focus">The row the last edit that applied leaves to follow; null after a removal.</param>
/// <param name="Refused">Why an edit was refused; the edits before it still applied.</param>
public sealed record BoardAnswer(BoardView View, IReadOnlyList<ItemRequirement>? Rows, long NextKey, bool Changed,
    IReadOnlyList<(long Old, long New)> Rekeyed, long? Focus, BoardRefusal? Refused);

/// <summary>One board edit (the EDIT of docs/requirement-editor.md), applied by the editor to the whole list.</summary>
public sealed class BoardEdit
{
    private readonly JsonObject json;
    private BoardEdit(string type, JsonObject? fields = null)
    {
        json = fields ?? new JsonObject();
        json["type"] = type;
    }

    /// <summary>The edit as its request writes it; a fresh node each time, so one edit may be sent twice.</summary>
    internal JsonNode Json => json.DeepClone();

    /// <summary>Rewrites the list into its canonical encoding, once when it is loaded or imported.</summary>
    public static BoardEdit Normalize() => new("normalize");
    /// <summary>Makes <paramref name="source"/> an either/or alternative of <paramref name="target"/>, any member of a chip or cluster.</summary>
    public static BoardEdit Join(long source, long target) => new("join", new() { ["source"] = source, ["target"] = target });
    /// <summary>Takes a cluster member out on its own; it leaves the cluster's stack behind.</summary>
    public static BoardEdit Detach(long key) => new("detach", new() { ["key"] = key });
    /// <summary>Removes a cluster member, or a chip's whole entry with its hidden copies.</summary>
    public static BoardEdit Remove(long key) => new("remove", new() { ["key"] = key });
    /// <summary>How many items the entry holding <paramref name="key"/> asks for.</summary>
    public static BoardEdit SetCount(long key, int count) => new("set_count", new() { ["key"] = key, ["count"] = Byte(count) });
    /// <summary>Sets or clears the combined level of the stack holding <paramref name="key"/>.</summary>
    public static BoardEdit SetTotal(long key, int? total) => new("set_total", new() { ["key"] = key, ["total"] = Byte(total) });
    /// <summary>Turns counting levels together on, at the stack's default total, or off.</summary>
    public static BoardEdit ToggleLevels(long key) => new("toggle_levels", new() { ["key"] = key });
    /// <summary>
    /// Stores <paramref name="requirement"/> with its stack's shape: onto the
    /// row <paramref name="key"/>, or appended as a new row when the key is
    /// null or names no row. The requirement's own row fields — its key and
    /// its group labels — are the editor's to keep.
    /// </summary>
    public static BoardEdit Save(long? key, ItemRequirement requirement, int count, int? total, int? copyDepth) => new("save", new()
    {
        ["key"] = key is long value ? Math.Max(0, value) : null,
        ["requirement"] = ResultsExport.EncodeRequirement(requirement),
        ["count"] = Byte(count), ["total"] = Byte(total), ["copy_depth"] = Byte(copyDepth),
    });

    /// <summary>The editor reads these counts as bytes; it clamps them to their own ranges.</summary>
    private static int Byte(int value) => Math.Clamp(value, 0, byte.MaxValue);
    private static int? Byte(int? value) => value is int number ? Byte(number) : null;
}

/// <summary>
/// The requirement board of one window, answered by the shared editor. The
/// editor is asked once per requirement list: redrawing the same list — the
/// floor slider does, on every step — reuses the answer, and an edit's answer
/// already carries the board of the list it leaves, which the redraw after it
/// then finds.
/// </summary>
public sealed class BoardEditor
{
    private string? request;
    private BoardView view = BoardView.Empty;

    /// <summary>How many times the editor was asked; the tests hold the memo to it.</summary>
    internal int Requests { get; private set; }

    /// <summary>
    /// The board of <paramref name="query"/>'s requirements. A list whose keys
    /// never went through <see cref="Load"/> is re-keyed by the editor, and
    /// the query adopts the repaired rows so that every key the board names
    /// is one its rows carry.
    /// </summary>
    public BoardView View(QuerySettings query)
    {
        var text = Request(query.Requirements, query, []);
        if (text == request) return view;
        var answer = Ask(text);
        if (answer.Rows is { } rows) query.Requirements = new(rows);
        Remember(query, answer);
        return answer.View;
    }

    /// <summary>
    /// Runs <paramref name="edits"/>, in order, on <paramref name="query"/>'s
    /// requirements. The answer carries the rows only when they changed, for
    /// the caller to adopt; the query itself is left alone.
    /// </summary>
    public BoardAnswer Edit(QuerySettings query, params BoardEdit[] edits)
    {
        var answer = Ask(Request(query.Requirements, query, edits));
        Remember(query, answer);
        return answer;
    }

    /// <summary>
    /// Takes in a list that was just loaded or imported: its rows are keyed
    /// 1…n afresh — keys are the board's own, never part of a document — and
    /// the editor puts the list in its canonical encoding, the way every edit
    /// leaves it.
    /// </summary>
    public void Load(QuerySettings query)
    {
        for (var index = 0; index < query.Requirements.Count; index++) query.Requirements[index].Key = index + 1;
        if (Edit(query, BoardEdit.Normalize()).Rows is { } rows) query.Requirements = new(rows);
    }

    /// <summary>Keeps <paramref name="answer"/>'s board as the board of the list it leaves.</summary>
    private void Remember(QuerySettings query, BoardAnswer answer)
    {
        request = Request(answer.Rows ?? query.Requirements, query, []);
        view = answer.View;
    }

    private BoardAnswer Ask(string text)
    {
        Requests++;
        return Answer(NativeEngine.RequirementBoard(text));
    }

    /// <summary>A board request: the rows, the edits to run on them, and the query's resin for the resin chip.</summary>
    internal static string Request(IEnumerable<ItemRequirement> rows, QuerySettings query, IReadOnlyCollection<BoardEdit> edits)
    {
        var request = new JsonObject { ["rows"] = new JsonArray([.. rows.Select(row => (JsonNode)ResultsExport.EncodeRow(row))]) };
        if (edits.Count > 0) request["edits"] = new JsonArray([.. edits.Select(edit => edit.Json)]);
        if (query.NeedsResin)
        {
            var filter = query.ArcaneResinFilter;
            // An amount out of range is the query's own problem to report
            // (QueryRelationships.Validate); the chip only has to draw.
            request["resin"] = new JsonObject
            {
                ["amount"] = query.ArcaneResinAuto ? (JsonNode)"auto" : Math.Clamp(query.ArcaneResin, 1, ushort.MaxValue),
                ["filter"] = new JsonObject
                {
                    ["uncursed"] = filter.Uncursed,
                    ["max_depth"] = filter.MaximumDepth is int depth and >= 0 and <= byte.MaxValue ? depth : null,
                    ["source"] = filter.Source is ScoutItemSource source ? ResultsExport.SourceName(source) : null,
                    ["include_mage_wand"] = filter.IncludeMageWand,
                },
            };
        }
        return request.ToJsonString();
    }

    /// <summary>Reads a board answer, or throws the editor's own reason when it could not read the request.</summary>
    /// <exception cref="RequirementEditorException"/>
    internal static BoardAnswer Answer(string text)
    {
        if (JsonNode.Parse(text) is not JsonObject answer)
            throw new RequirementEditorException("The requirement editor's answer could not be read.");
        if (answer["error"] is JsonNode error) throw new RequirementEditorException((string?)error ?? "", (long?)answer["key"]);
        var changed = (bool)answer["changed"]!;
        var counts = answer["counts"]!;
        var view = new BoardView(
            [.. answer["items"]!.AsArray().Select(entry => Entry(entry!))],
            new((int)counts["ordinary"]!, (int)counts["blanket"]!),
            [.. answer["problems"]!.AsArray().Select(problem => new BoardProblem((string)problem!["message"]!, Keys(problem["keys"]),
                (string?)problem["scope"] switch { "row" => ProblemScope.Row, "group" => ProblemScope.Group, _ => ProblemScope.List }))],
            answer["resin"] is JsonObject resin ? Resin(resin) : null);
        return new(view,
            changed ? [.. answer["rows"]!.AsArray().Select(row => ResultsExport.DecodeRow(row!.AsObject()))] : null,
            (long)answer["next_key"]!, changed,
            [.. answer["rekeyed"]!.AsArray().Select(pair => ((long)pair![0]!, (long)pair[1]!))],
            (long?)answer["focus"],
            answer["refused"] is JsonObject refused ? new((string)refused["reason"]!, (string)refused["message"]!) : null);
    }

    /// <summary>One ITEM of the answer: a board entry.</summary>
    private static BoardEntry Entry(JsonNode entry)
    {
        var stack = entry["stack"]!;
        return new((string)entry["id"]!, (bool)entry["blanket"]!, (int?)entry["cluster"], (string?)entry["label"],
            Keys(entry["members"]), Keys(entry["extras"]),
            new((int)stack["count"]!, (int)stack["max"]!, (bool)stack["can_grow"]!, (bool)stack["can_change_count"]!,
                (int?)stack["total"], (bool)stack["can_count_levels"]!, (int)stack["level_capacity"]!,
                (int)stack["default_total"]!, (int?)stack["copy_depth"], (bool)stack["can_set_copy_depth"]!,
                (string)stack["count_text"]!, (string)stack["total_text"]!),
            Badge(entry["badges"]!["count"]), Badge(entry["badges"]!["total"]),
            [.. entry["chips"]!.AsArray().Select(chip => Chip(chip!))], (string?)entry["problem"]);
    }

    private static BoardChip Chip(JsonNode chip) => new(
        (long)chip["key"]!, (string)chip["name"]!, (string)chip["title"]!, (string?)chip["item"],
        ResultsExport.KindNamed((string?)chip["kind"]), Tags(chip["tags"]), Tags(chip["trailing_tags"]),
        chip["effect"] is JsonObject effect
            ? new((string)effect["label"]!, Strings(effect["effects"]), (bool)effect["any_enchantment"]!, (bool)effect["curses_only"]!)
            : null,
        (bool)chip["uncursed"]!, Strings(chip["details"]),
        [.. chip["relations"]!.AsArray().Select(relation => new ChipRelation(
            (string)relation!["glyph"]! switch { "or" => RelationGlyph.Or, "sum" => RelationGlyph.Sum, _ => RelationGlyph.Times },
            (string)relation["text"]!))],
        (string)chip["description"]!, (string?)chip["problem"], (bool)chip["in_cluster"]!, (bool)chip["can_detach"]!,
        Keys(chip["join"]),
        [.. chip["refuse"]!.AsArray().Select(refusal => new JoinRefusal((long)refusal!["key"]!, (string)refusal["reason"]!, (string)refusal["message"]!))]);

    private static ResinChip Resin(JsonObject resin) => new(
        (string)resin["name"]!, Tags(resin["tags"]), (bool)resin["uncursed"]!, (string?)resin["tooltip"],
        (string?)resin["amount_tooltip"], Strings(resin["details"]), (string)resin["description"]!);

    private static BoardBadge? Badge(JsonNode? badge) => badge is JsonObject
        ? new((string)badge["text"]!, (string)badge["compact_text"]!, (string)badge["tooltip"]!)
        : null;

    private static IReadOnlyList<ChipTag> Tags(JsonNode? tags) =>
        [.. (tags as JsonArray ?? []).Select(tag => new ChipTag((string)tag!["text"]!, (string?)tag["style"] == "upgrade"))];

    private static IReadOnlyList<long> Keys(JsonNode? keys) => [.. (keys as JsonArray ?? []).Select(key => (long)key!)];

    private static IReadOnlyList<string> Strings(JsonNode? values) => [.. (values as JsonArray ?? []).Select(value => (string)value!)];
}
