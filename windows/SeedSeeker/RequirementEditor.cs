using System.Text.Json.Nodes;

namespace SeedSeeker;

// The requirement board and its sheet as the shared core's editor answers
// them (crates/seedfinder-core/src/editor, reached through
// seedfinder_requirement_board and seedfinder_requirement_editor; the format
// is docs/requirement-editor.md). Every rule of the board lives there — how
// the flat list folds into chips, clusters and stacks, what a drop, a badge
// or a menu writes back, what each chip and badge says, and what is wrong
// with the list — and so does every rule of the sheet: what each control
// offers, shows and says, what a change resets, and what a save writes. This
// file only types the envelopes for the window, and the window only draws
// them. Like Models.cs it must stay free of Windows App SDK types:
// SeedSeeker.Tests links it to run on any host.

/// <summary>A request the shared requirement editor could not read, with the row at fault when one was.</summary>
public sealed class RequirementEditorException(string message, long? key = null) : Exception(message)
{
    public long? Key { get; } = key;
}

/// <summary>How a tag is tinted: a plain qualifier, the upgrade, or resin the chip counts (the resin chip's amount and <c>Mage +2</c>).</summary>
public enum TagStyle { Plain, Upgrade, Credit }

/// <summary>A tiny qualifier beside a chip's name, tinted by its style, with its own hover text when it has one.</summary>
public sealed record ChipTag(string Text, TagStyle Style = TagStyle.Plain, string? Tooltip = null);

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
/// One visible row: a lone chip, or one member of a cluster, with its own
/// stack. Every badge and stepper is a chip's: a cluster member's stack is its
/// own, drawn on its chip inside the cluster's outline, and nothing is drawn
/// or counted for the cluster as a whole.
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
/// <param name="CountBadge">The <c>×N</c> badge, when the chip asks for more than one item.</param>
/// <param name="TotalBadge">The <c>Σ ≥ T</c> badge, when the chip counts levels together.</param>
/// <param name="RemainingBadges">The badges the chip keeps while one item is lifted away; null when it has no copies, so the whole chip leaves.</param>
/// <param name="Copies">The hidden copies' keys behind the chip's badge; members whose stacks are alike share theirs.</param>
/// <param name="CanDetach">Whether "On its own" applies: the chip is a cluster member.</param>
/// <param name="Join">The visible rows this chip may join, in list order.</param>
/// <param name="Refuse">The visible rows a join onto is refused, with the reason.</param>
public sealed record BoardChip(long Key, string Name, string Title, string? Item, ItemKind? Kind,
    IReadOnlyList<ChipTag> Tags, IReadOnlyList<ChipTag> TrailingTags, ChipEffect? Effect, bool Uncursed,
    IReadOnlyList<string> Details, IReadOnlyList<ChipRelation> Relations, string Description, string? Problem,
    BoardBadge? CountBadge, BoardBadge? TotalBadge, BoardBadges? RemainingBadges, IReadOnlyList<long> Copies, BoardStack Stack,
    bool InCluster, bool CanDetach, IReadOnlyList<long> Join, IReadOnlyList<JoinRefusal> Refuse)
{
    /// <summary>
    /// The chip as a drag picks it up: every drag moves one item, so the
    /// chip that rides under the pointer is that one item — its name and
    /// tags, without its <c>×N</c> or <c>Σ</c> badges.
    /// </summary>
    public BoardChip Lifted => this with { CountBadge = null, TotalBadge = null };

    /// <summary>
    /// The chip as a drag leaves it at its origin: its stack one item fewer,
    /// with the badges that rest keeps (<see cref="RemainingBadges"/>); null
    /// when the whole chip leaves, and the origin stays as it was.
    /// </summary>
    public BoardChip? LeftBehind => RemainingBadges is { } rest ? this with { CountBadge = rest.Count, TotalBadge = rest.Total } : null;

    /// <summary>
    /// The chip's hover detail: its title, what it asks of one item, the
    /// relationships it stands in, and the problem it has, one line each.
    /// </summary>
    public string Detail => string.Join("\n", new[] { Title, Details.Count > 0 ? string.Join(" · ", Details) : null }
        .Concat(Relations.Select(relation => $"{relation.Glyph switch { RelationGlyph.Or => "or", RelationGlyph.Sum => "Σ", _ => "×" }} {relation.Text}"))
        .Append(Problem).OfType<string>());
}

/// <summary>
/// What a chip's count and combined-level steppers offer: how many items it
/// asks for, whether that may grow or change, its combined level (a lone ring
/// stack's only) and the highest one its items can reach, and the floor limit
/// of its hidden copies.
/// </summary>
/// <param name="CountMax">The count stepper's upper bound: <paramref name="Max"/> while the entry can grow, else its count.</param>
public sealed record BoardStack(int Count, int Max, bool CanGrow, bool CanChangeCount, int CountMax, int? Total, bool CanCountLevels,
    int LevelCapacity, int DefaultTotal, int? CopyDepth, bool CanSetCopyDepth, string CountText, string TotalText);

/// <summary>A badge's words at rest: <c>×3</c> or <c>Σ ≥ 5</c>, and what it means.</summary>
public sealed record BoardBadge(string Text, string CompactText, string Tooltip);

/// <summary>A chip's pair of badges, either of them absent: its <c>×N</c> and its <c>Σ ≥ T</c>.</summary>
public sealed record BoardBadges(BoardBadge? Count, BoardBadge? Total);

/// <summary>
/// One board entry: a chip, or an either/or cluster of chips. The entry has
/// no badge or stepper of its own: they are its chips'.
/// </summary>
/// <param name="Id">Stable while the entry survives an edit: <c>r17</c> for a chip, <c>c3</c> for a cluster.</param>
/// <param name="Cluster">The alternative group of a cluster of two or more; null for a chip.</param>
/// <param name="Label">A cluster's caption, <c>Any of 2</c>.</param>
/// <param name="Name">What a menu calls the entry: a chip's name, or a cluster's (<c>Spear or Mace</c>).</param>
/// <param name="Members">The visible rows' keys: one for a chip, every member of a cluster.</param>
/// <param name="Extras">Every hidden copy's key behind any of its chips' badges, each once.</param>
/// <param name="Problem">The first problem touching any member or hidden copy.</param>
public sealed record BoardEntry(string Id, bool Blanket, int? Cluster, string? Label, string Name, IReadOnlyList<long> Members,
    IReadOnlyList<long> Extras, IReadOnlyList<BoardChip> Chips, string? Problem);

public enum ProblemScope { Row, Group, List }

/// <summary>One thing the list must fix before it can be searched, and the rows it blames.</summary>
public sealed record BoardProblem(string Message, IReadOnlyList<long> Keys, ProblemScope Scope);

/// <summary>
/// The Arcane Resin chip: <c>Auto</c> or <c>≥N</c> and <c>Mage +2</c>, the
/// resin it counts (styled <see cref="TagStyle.Credit"/>, each explaining
/// itself where it needs to), then the donors' <c>F≤N</c>; and the donors in words.
/// </summary>
/// <param name="Tooltip">The donors' source, the one filter no tag shows; null for any source.</param>
public sealed record ResinChip(string Name, IReadOnlyList<ChipTag> Tags, bool Uncursed, string? Tooltip,
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

/// <summary>What a drop sends: a join, a detach, or one item taken away (<c>remove_one</c>); a refusal sends nothing.</summary>
public enum DropEffect { None, Join, Refused, Detach, RemoveOne }

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
    /// What "Either/or with…" offers the chip keyed <paramref name="key"/>:
    /// each entry it may join once, in list order, under the entry's name, with
    /// the member a join names — any member joins the whole cluster.
    /// </summary>
    public IReadOnlyList<(string Name, long Target)> JoinChoices(long key) => ChipOf(key) is { } chip
        ? [.. chip.Join.Select(target => (Entry: EntryOf(target), Target: target)).Where(choice => choice.Entry is not null)
            .GroupBy(choice => choice.Entry!.Id).Select(group => (group.First().Entry!.Name, group.First().Target))]
        : [];

    /// <summary>
    /// What dropping the chip keyed <paramref name="source"/> does, decided
    /// from its chip as the editor answered it: onto a chip or a cluster, a
    /// join the editor offers, or the refusal it gives; onto the empty board of
    /// the chip's own section (<paramref name="blanketBoard"/>), a cluster
    /// member comes out on its own while a lone chip stays where it is; onto
    /// the remove zone, one item of the chip is taken away. Every drag moves
    /// one item, so a stack gives up one copy, whichever the gesture. Anything
    /// else does nothing.
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
            DropKind.Remove => new(DropEffect.RemoveOne),
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

/// <summary>
/// One board edit (the EDIT of docs/requirement-editor.md), applied by the
/// editor to the whole list. The sheet saves through <see cref="RequirementSheet.Save"/>.
/// </summary>
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
    /// <summary>
    /// Makes one item of <paramref name="source"/> an either/or alternative of
    /// <paramref name="target"/>, any member of a chip or cluster: the rest of
    /// a stacked source stays where it was, and a stacked target keeps its
    /// stack as a member.
    /// </summary>
    public static BoardEdit Join(long source, long target) => new("join", new() { ["source"] = source, ["target"] = target });
    /// <summary>Takes one item of a cluster member out on its own; the rest of its stack stays in the cluster.</summary>
    public static BoardEdit Detach(long key) => new("detach", new() { ["key"] = key });
    /// <summary>
    /// Removes the chip with its whole stack: a cluster member with its own
    /// copies, or a lone chip's whole entry. The chip menu's Remove.
    /// </summary>
    public static BoardEdit Remove(long key) => new("remove", new() { ["key"] = key });
    /// <summary>
    /// Takes one item of the chip away, as a drop on the remove zone does: a
    /// stack keeps the rest, one fewer; a chip of one item is removed.
    /// </summary>
    public static BoardEdit RemoveOne(long key) => new("remove_one", new() { ["key"] = key });
    /// <summary>How many items the chip <paramref name="key"/> asks for: a lone chip's stack, or a cluster member's own.</summary>
    public static BoardEdit SetCount(long key, int count) => new("set_count", new() { ["key"] = key, ["count"] = Byte(count) });
    /// <summary>Sets or clears the combined level of the lone ring stack <paramref name="key"/>.</summary>
    public static BoardEdit SetTotal(long key, int? total) => new("set_total", new() { ["key"] = key, ["total"] = Byte(total) });
    /// <summary>Turns counting levels together on, at the stack's default total, or off.</summary>
    public static BoardEdit ToggleLevels(long key) => new("toggle_levels", new() { ["key"] = key });

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

    /// <summary>
    /// Why <paramref name="query"/> cannot be searched or shared, or null: the
    /// query's own settings first, then the first problem of its list.
    /// </summary>
    public string? Problem(QuerySettings query) => QueryRelationships.Validate(query) ?? View(query).Problem;

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
        var request = new JsonObject { ["rows"] = Rows(rows) };
        if (edits.Count > 0) request["edits"] = new JsonArray([.. edits.Select(edit => edit.Json)]);
        if (ResinRequest(query) is { } resin) request["resin"] = resin;
        return request.ToJsonString();
    }

    /// <summary>The rows of a request, both sections in list order.</summary>
    internal static JsonArray Rows(IEnumerable<ItemRequirement> rows) => new([.. rows.Select(row => (JsonNode)ResultsExport.EncodeRow(row))]);

    /// <summary>The query's Arcane Resin condition as the editor reads it (RESIN), or null when the query asks for none.</summary>
    internal static JsonObject? ResinRequest(QuerySettings query)
    {
        if (!query.NeedsResin) return null;
        var filter = query.ArcaneResinFilter;
        // An amount out of range is the query's own problem to report
        // (QueryRelationships.Validate); the chip only has to draw.
        return new JsonObject
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

    /// <summary>Reads a board answer, or throws the editor's own reason when it could not read the request.</summary>
    /// <exception cref="RequirementEditorException"/>
    internal static BoardAnswer Answer(string text)
    {
        var answer = Parse(text);
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
            (long)answer["next_key"]!, changed, Rekeyed(answer["rekeyed"]), (long?)answer["focus"],
            answer["refused"] is JsonObject refused ? new((string)refused["reason"]!, (string)refused["message"]!) : null);
    }

    /// <summary>One ITEM of the answer: a board entry.</summary>
    private static BoardEntry Entry(JsonNode entry) => new((string)entry["id"]!, (bool)entry["blanket"]!, (int?)entry["cluster"],
        (string?)entry["label"], (string)entry["name"]!, Keys(entry["members"]), Keys(entry["extras"]),
        [.. entry["chips"]!.AsArray().Select(chip => Chip(chip!))], (string?)entry["problem"]);

    /// <summary>One CHIP of an answer: a visible row, or the chip a sheet would save.</summary>
    internal static BoardChip Chip(JsonNode chip) => new(
        (long)chip["key"]!, (string)chip["name"]!, (string)chip["title"]!, (string?)chip["item"],
        ResultsExport.KindNamed((string?)chip["kind"]), Tags(chip["tags"]), Tags(chip["trailing_tags"]),
        chip["effect"] is JsonObject effect
            ? new((string)effect["label"]!, Strings(effect["effects"]), (bool)effect["any_enchantment"]!, (bool)effect["curses_only"]!)
            : null,
        (bool)chip["uncursed"]!, Strings(chip["details"]),
        [.. chip["relations"]!.AsArray().Select(relation => new ChipRelation(
            (string)relation!["glyph"]! switch { "or" => RelationGlyph.Or, "sum" => RelationGlyph.Sum, _ => RelationGlyph.Times },
            (string)relation["text"]!))],
        (string)chip["description"]!, (string?)chip["problem"],
        Badge(chip["badges"]!["count"]), Badge(chip["badges"]!["total"]),
        chip["remaining_badges"] is JsonObject rest ? new(Badge(rest["count"]), Badge(rest["total"])) : null,
        Keys(chip["copies"]), Stack(chip["stack"]!),
        (bool)chip["in_cluster"]!, (bool)chip["can_detach"]!,
        Keys(chip["join"]),
        [.. chip["refuse"]!.AsArray().Select(refusal => new JoinRefusal((long)refusal!["key"]!, (string)refusal["reason"]!, (string)refusal["message"]!))]);

    /// <summary>A chip's STACK: what its count, combined-level and copy-floor steppers offer.</summary>
    private static BoardStack Stack(JsonNode stack) => new((int)stack["count"]!, (int)stack["max"]!, (bool)stack["can_grow"]!,
        (bool)stack["can_change_count"]!, (int)stack["count_max"]!, (int?)stack["total"], (bool)stack["can_count_levels"]!,
        (int)stack["level_capacity"]!, (int)stack["default_total"]!, (int?)stack["copy_depth"], (bool)stack["can_set_copy_depth"]!,
        (string)stack["count_text"]!, (string)stack["total_text"]!);

    private static ResinChip Resin(JsonObject resin) => new(
        (string)resin["name"]!, Tags(resin["tags"]), (bool)resin["uncursed"]!, (string?)resin["tooltip"],
        Strings(resin["details"]), (string)resin["description"]!);

    private static BoardBadge? Badge(JsonNode? badge) => badge is JsonObject
        ? new((string)badge["text"]!, (string)badge["compact_text"]!, (string)badge["tooltip"]!)
        : null;

    private static IReadOnlyList<ChipTag> Tags(JsonNode? tags) =>
        [.. (tags as JsonArray ?? []).Select(tag => new ChipTag((string)tag!["text"]!,
            (string?)tag["style"] switch { "upgrade" => TagStyle.Upgrade, "credit" => TagStyle.Credit, _ => TagStyle.Plain },
            (string?)tag["tooltip"]))];

    private static IReadOnlyList<long> Keys(JsonNode? keys) => [.. (keys as JsonArray ?? []).Select(key => (long)key!)];

    internal static IReadOnlyList<string> Strings(JsonNode? values) => [.. (values as JsonArray ?? []).Select(value => (string)value!)];

    /// <summary>The keys the editor repaired, old to new.</summary>
    internal static IReadOnlyList<(long Old, long New)> Rekeyed(JsonNode? pairs) =>
        [.. (pairs as JsonArray ?? []).Select(pair => ((long)pair![0]!, (long)pair[1]!))];

    /// <summary>Reads an answer's JSON, or throws the editor's own reason when it could not read the request.</summary>
    /// <exception cref="RequirementEditorException"/>
    internal static JsonObject Parse(string text)
    {
        if (JsonNode.Parse(text) is not JsonObject answer)
            throw new RequirementEditorException("The requirement editor's answer could not be read.");
        if (answer["error"] is JsonNode error) throw new RequirementEditorException((string?)error ?? "", (long?)answer["key"]);
        return answer;
    }
}

/// <summary>
/// One choice of a sheet picker: the value a change sends back, its words,
/// the heading it sits under (<c>Tier 2</c> in the weapon list), and whether
/// it is offered only because the draft already names it — an imported tier-1
/// item — so it shows and saves back unchanged.
/// </summary>
public sealed record SheetOption<T>(T Value, string Label, string? Group = null, bool Hidden = false);

/// <summary>A picker: whether the sheet shows it, the chosen value, and every choice in order.</summary>
public sealed record SheetChoice<T>(bool Visible, T Value, IReadOnlyList<SheetOption<T>> Options)
{
    /// <summary>The chosen option's position, or -1 when no option holds the value.</summary>
    public int Selected => IndexOf(Options, Value);

    /// <summary>Whether <paramref name="other"/> lists the same choices, so a picker drawn for it can keep its items.</summary>
    public bool SameOptions(SheetChoice<T>? other) => other is not null && other.Options.SequenceEqual(Options);

    internal static int IndexOf(IReadOnlyList<SheetOption<T>> options, T value)
    {
        for (var index = 0; index < options.Count; index++)
            if (EqualityComparer<T>.Default.Equals(options[index].Value, value)) return index;
        return -1;
    }
}

/// <summary>
/// A filter with a mode and a value — the tier, the upgrade: the modes its
/// picker offers, whether its value slider shows, and the slider's range and
/// words (<c>Tier 3 or higher</c>, <c>+2</c>). The value is always within
/// Min…Max, even while the mode is "any" and the slider hidden.
/// </summary>
public sealed record SheetModeRange(bool Visible, string Mode, IReadOnlyList<SheetOption<string>> Modes, bool ValueVisible,
    int Value, int Min, int Max, string ValueLabel)
{
    /// <summary>The mode picker.</summary>
    public SheetChoice<string> Picker => new(Visible, Mode, Modes);
}

/// <summary>A check box, and the help text under it whenever it shows (null for none).</summary>
public sealed record SheetToggle(bool Visible, bool Value, string Label, string? Caption = null);

/// <summary>
/// A switch with a floor slider — the item's floor limit, the copies', the
/// resin donors'. The slider runs over <see cref="Options"/>, which skip the
/// empty boss floors, and the value is always one of them.
/// </summary>
public sealed record SheetFloor(bool Visible, bool Enabled, int Value, IReadOnlyList<SheetOption<int>> Options, string Label, string ValueLabel)
{
    /// <summary>The value's position among the options: where the slider sits.</summary>
    public int Selected => SheetChoice<int>.IndexOf(Options, Value);

    /// <summary>The floor at slider position <paramref name="position"/>, the nearest option's.</summary>
    public int At(double position) => Options[Math.Clamp((int)Math.Round(position), 0, Options.Count - 1)].Value;

    public bool ShowsValue => Visible && Enabled;
}

/// <summary>
/// A switch with a stepper — the transmutations, the combined level — whose
/// value is always within Min…Max, and its help text, shown while
/// <see cref="CaptionVisible"/>.
/// </summary>
public sealed record SheetStepper(bool Visible, bool Enabled, int Value, int Min, int Max, string Label, string? Caption,
    bool CaptionVisible, string ValueLabel)
{
    public bool ShowsValue => Visible && Enabled;
}

/// <summary>One effect of the "Specific…" grid: an enchantment (a glyph, on armor) or a curse.</summary>
public sealed record SheetEffectChoice(string Value, string Label, bool Curse, bool Selected);

/// <summary>
/// The effect filter of a weapon or armor: its section's label
/// (<c>Enchantment</c>, <c>Glyph</c>), its mode, and the grid of effects
/// "Specific…" ticks from — shown while <see cref="ChoicesVisible"/>, curses
/// listed only while the item may be cursed — under their headings, with what
/// the ticked ones mean.
/// </summary>
public sealed record SheetEffect(bool Visible, string Label, string Mode, IReadOnlyList<SheetOption<string>> Modes, bool ChoicesVisible,
    IReadOnlyList<SheetEffectChoice> Choices, IReadOnlyList<SheetOption<string>> Groups, string Caption)
{
    /// <summary>The mode picker: Any, Any enchantment (Any glyph), Specific….</summary>
    public SheetChoice<string> Picker => new(Visible, Mode, Modes);

    /// <summary>The heading over the enchantments (<c>Glyphs</c> on armor) or the curses; null when the grid lists none.</summary>
    public string? Heading(bool curse) => Groups.FirstOrDefault(group => group.Value == (curse ? "curse" : "enchantment"))?.Label;

    /// <summary>Whether <paramref name="other"/> lists the same effects, ticked or not, so a grid drawn for it can keep its boxes.</summary>
    public bool SameChoices(SheetEffect? other) => other is not null
        && other.Choices.Select(choice => (choice.Value, choice.Label, choice.Curse)).SequenceEqual(Choices.Select(choice => (choice.Value, choice.Label, choice.Curse)));
}

/// <summary>
/// The stack section: how many items the chip asks for (<see cref="Label"/>
/// its stepper's), the copies' floor limit, and the combined level.
/// </summary>
public sealed record SheetStack(bool Visible, string Label, int Count, int Min, int Max, string ValueLabel, SheetFloor CopyDepth, SheetStepper CountLevels);

/// <summary>
/// The Arcane Resin section, shown while the resin is the picked item: its
/// label, which the amount field takes too; the Amount/Auto choice; what Auto
/// means (<see cref="Caption"/>, shown in the amount field's place); the
/// amount as typed (null for an empty field) and the amounts that save; and
/// the Mage's wand credit. The donors' uncursed, source and floor filters are
/// the sheet's own controls meanwhile.
/// </summary>
public sealed record SheetResin(bool Visible, string Label, bool Auto, IReadOnlyList<SheetOption<bool>> Modes, string Caption,
    double? Amount, int Min, int Max, SheetToggle IncludeMageWand)
{
    /// <summary>The Amount/Auto picker.</summary>
    public SheetChoice<bool> Picker => new(Visible, Auto, Modes);
}

/// <summary>What a sheet was opened on: a new chip, a row on the board, or the query's Arcane Resin.</summary>
public enum SheetOrigin { New, Row, Resin }

/// <summary>
/// Everything a requirement sheet shows, as the editor answers it (FORM):
/// every control's visibility, value, range, options and words, the chip a
/// save would produce, and why it cannot save yet. The dialog's own chrome —
/// its title and buttons — follows <see cref="IsNew"/>, <see cref="Origin"/>,
/// <see cref="Blanket"/> and <see cref="ResinPicked"/>.
/// </summary>
public sealed record SheetForm
{
    /// <summary>The sheet adds a chip, or resin the query has none of, rather than editing one.</summary>
    public required bool IsNew { get; init; }
    public required SheetOrigin Origin { get; init; }
    /// <summary>The row the sheet was opened on, for <see cref="SheetOrigin.Row"/>.</summary>
    public required long? OriginKey { get; init; }
    public required bool Blanket { get; init; }
    public required bool InCluster { get; init; }
    /// <summary>Arcane Resin is the picked item: the sheet edits the query's resin.</summary>
    public required bool ResinPicked { get; init; }
    /// <summary>What the sheet is about: the requirement's title, or <c>Arcane Resin</c>.</summary>
    public required string Title { get; init; }
    /// <summary>The chip a save would put on the board, or null while there are errors or the resin is picked.</summary>
    public required BoardChip? Preview { get; init; }
    /// <summary>The six families.</summary>
    public required SheetChoice<string> Category { get; init; }
    /// <summary>The flat list of kinds: the families, with melee and thrown weapons among them.</summary>
    public required SheetChoice<string> Kind { get; init; }
    /// <summary>Any, melee or thrown, on weapons.</summary>
    public required SheetChoice<string> WeaponType { get; init; }
    /// <summary>The wildcard (null) unless the family always names one, Arcane Resin when offered, then the items.</summary>
    public required SheetChoice<string?> Item { get; init; }
    public required SheetModeRange Tier { get; init; }
    public required SheetModeRange Upgrade { get; init; }
    public required SheetEffect Effect { get; init; }
    public required SheetToggle Uncursed { get; init; }
    /// <summary>Any source (null), then every source.</summary>
    public required SheetChoice<string?> Source { get; init; }
    public required SheetFloor FloorLimit { get; init; }
    public required SheetToggle ExcludeResin { get; init; }
    public required SheetStepper Transmutations { get; init; }
    public required SheetToggle SelectTrinket { get; init; }
    public required SheetStack Stack { get; init; }
    public required SheetResin Resin { get; init; }
    /// <summary>Why the draft cannot be saved, in the order to show them.</summary>
    public required IReadOnlyList<string> Errors { get; init; }
    public required bool CanSave { get; init; }

    /// <summary>
    /// Whether the sheet edits the resin <paramref name="query"/> already asks
    /// for, so the dialog says Save and offers Remove: the resin chip's sheet,
    /// or Arcane Resin picked on a new sheet while the query has resin. The
    /// editor answers the second as new, though the section starts from that
    /// resin and its save replaces it; main's resin dialog, which a new
    /// requirement's Arcane Resin button opened, said Save and Remove there.
    /// </summary>
    public bool EditsQueryResin(QuerySettings query) =>
        Origin == SheetOrigin.Resin || (Origin == SheetOrigin.New && ResinPicked && query.NeedsResin);
}

/// <summary>One control the user moved (the CHANGE of docs/requirement-editor.md); its value is the form's own.</summary>
public sealed class SheetChange
{
    private readonly string type;
    private readonly JsonNode? value;
    private SheetChange(string type, JsonNode? value) { this.type = type; this.value = value; }

    /// <summary>The change as its request writes it; a fresh node each time.</summary>
    internal JsonObject Json => new() { ["type"] = type, ["value"] = value?.DeepClone() };

    public override string ToString() => Json.ToJsonString();

    public static SheetChange SetCategory(string family) => new("set_category", family);
    public static SheetChange SetWeaponType(string type) => new("set_weapon_type", type);
    /// <summary>The flat kind picker: a family, <c>melee_weapon</c> or <c>thrown_weapon</c>.</summary>
    public static SheetChange SetKind(string kind) => new("set_kind", kind);
    /// <summary>An item's id, <c>arcane_resin</c>, or null for the wildcard.</summary>
    public static SheetChange SetItem(string? item) => new("set_item", item);
    public static SheetChange SetTierMode(string mode) => new("set_tier_mode", mode);
    public static SheetChange SetTier(int tier) => new("set_tier", Byte(tier));
    public static SheetChange SetUpgradeMode(string mode) => new("set_upgrade_mode", mode);
    public static SheetChange SetUpgrade(int upgrade) => new("set_upgrade", Byte(upgrade));
    public static SheetChange SetEffectMode(string mode) => new("set_effect_mode", mode);
    /// <summary>Ticks or unticks one effect of the "Specific…" grid.</summary>
    public static SheetChange ToggleEffect(string effect) => new("toggle_effect", effect);
    public static SheetChange SetUncursed(bool uncursed) => new("set_uncursed", uncursed);
    /// <summary>A source's name, or null for any source.</summary>
    public static SheetChange SetSource(string? source) => new("set_source", source);
    public static SheetChange SetFloorLimitEnabled(bool enabled) => new("set_floor_limit_enabled", enabled);
    public static SheetChange SetFloorLimit(int floor) => new("set_floor_limit", Byte(floor));
    public static SheetChange SetExcludeResin(bool exclude) => new("set_exclude_resin", exclude);
    public static SheetChange SetTransmutationsEnabled(bool enabled) => new("set_transmutations_enabled", enabled);
    public static SheetChange SetTransmutations(int count) => new("set_transmutations", Byte(count));
    public static SheetChange SetSelectTrinket(bool select) => new("set_select_trinket", select);
    public static SheetChange SetCount(int count) => new("set_count", Byte(count));
    public static SheetChange SetCopyDepthEnabled(bool enabled) => new("set_copy_depth_enabled", enabled);
    public static SheetChange SetCopyDepth(int floor) => new("set_copy_depth", Byte(floor));
    public static SheetChange SetCountLevels(bool enabled) => new("set_count_levels", enabled);
    public static SheetChange SetTotal(int total) => new("set_total", Byte(total));
    public static SheetChange SetResinAuto(bool auto) => new("set_resin_auto", auto);
    /// <summary>The amount as typed; an empty or unreadable field (NaN) is no amount.</summary>
    public static SheetChange SetResinAmount(double amount) => new("set_resin_amount", double.IsFinite(amount) ? amount : null);
    public static SheetChange SetIncludeMageWand(bool include) => new("set_include_mage_wand", include);

    /// <summary>The editor reads these values as bytes; it clamps them to their own ranges.</summary>
    private static int Byte(int value) => Math.Clamp(value, 0, byte.MaxValue);
}

/// <summary>The query's Arcane Resin condition as a sheet saves it: Auto, or at least <see cref="Amount"/>.</summary>
public sealed record ResinCondition(bool Auto, int Amount, ArcaneResinFilter Filter);

/// <summary>
/// What a sheet's save did.
/// </summary>
/// <param name="Rows">The list after the save, only when it changed: an unchanged
/// chip saved as it was is never written back.</param>
/// <param name="Rekeyed">Keys the editor repaired, old to new.</param>
/// <param name="Focus">The row of the chip the save landed in, to return to; null when the sheet saved the resin.</param>
/// <param name="Resin">The query's new resin condition, when Arcane Resin was the picked item and the save
/// sets it; null leaves the query's resin as it is, as the resin chip's sheet saved untouched does.</param>
/// <param name="ClearResin">The resin chip was saved as a requirement: the query drops its resin.</param>
public sealed record SheetSave(IReadOnlyList<ItemRequirement>? Rows, long NextKey, IReadOnlyList<(long Old, long New)> Rekeyed,
    long? Focus, ResinCondition? Resin, bool ClearResin)
{
    /// <summary>
    /// Adopts the save into <paramref name="query"/>: its rows when they
    /// changed, and what becomes of the query's resin. Whether anything
    /// changed, so an unchanged save is neither redrawn nor written.
    /// </summary>
    public bool ApplyTo(QuerySettings query)
    {
        var changed = false;
        if (Rows is { } rows) { query.Requirements = new(rows); changed = true; }
        var (auto, amount, filter) = ClearResin ? (false, 0, new ArcaneResinFilter())
            : Resin is { } resin ? (resin.Auto, resin.Auto ? 0 : resin.Amount, resin.Filter)
            : (query.ArcaneResinAuto, query.ArcaneResin, query.ArcaneResinFilter);
        if (auto != query.ArcaneResinAuto || amount != query.ArcaneResin || filter != query.ArcaneResinFilter)
        {
            query.ArcaneResinAuto = auto; query.ArcaneResin = amount; query.ArcaneResinFilter = filter;
            changed = true;
        }
        return changed;
    }
}

/// <summary>
/// One open requirement sheet: the draft the editor holds between requests —
/// kept as the opaque string it answered and sent back untouched — and the
/// form it last answered for it. A sheet opens on a chip, a new chip or the
/// query's Arcane Resin, takes one change per control the user moves, and
/// saves onto the list as it stands.
/// </summary>
public sealed class RequirementSheet
{
    private string draft;

    private RequirementSheet(JsonObject answer) => (draft, Form) = Read(answer);

    /// <summary>What the sheet shows now.</summary>
    public SheetForm Form { get; private set; }

    /// <summary>
    /// Opens a sheet on <paramref name="query"/>'s requirements: on the
    /// visible row <paramref name="key"/>, or on a new chip of the blanket or
    /// ordinary section when it is null; with <paramref name="openResin"/>, on
    /// the query's Arcane Resin. The resin section starts from the query's
    /// resin, and <paramref name="offerResin"/> offers Arcane Resin among the
    /// wands of an ordinary sheet.
    /// </summary>
    /// <exception cref="RequirementEditorException">The row cannot be read, so it has no sheet.</exception>
    public static RequirementSheet Open(QuerySettings query, long? key, bool blanket = false, bool offerResin = false, bool openResin = false) =>
        new(Ask(new JsonObject
        {
            ["op"] = "open", ["rows"] = BoardEditor.Rows(query.Requirements), ["key"] = key is > 0 ? key : null,
            ["blanket"] = blanket, ["resin"] = BoardEditor.ResinRequest(query), ["offer_resin"] = offerResin, ["open_resin"] = openResin,
        }));

    /// <summary>Applies one control the user moved; a change to a control the form hides changes nothing.</summary>
    /// <exception cref="RequirementEditorException">The editor could not read the change.</exception>
    public void Change(SheetChange change) =>
        (draft, Form) = Read(Ask(new JsonObject { ["op"] = "change", ["draft"] = draft, ["change"] = change.Json }));

    /// <summary>
    /// Saves the draft onto <paramref name="query"/>'s requirements as they
    /// stand, or refuses it: null, with <see cref="Form"/>'s errors saying why.
    /// </summary>
    /// <exception cref="RequirementEditorException">The editor could not read the rows.</exception>
    public SheetSave? Save(QuerySettings query)
    {
        var answer = Ask(new JsonObject { ["op"] = "save", ["draft"] = draft, ["rows"] = BoardEditor.Rows(query.Requirements) });
        if (answer["saved"] is not JsonObject saved)
        {
            (draft, Form) = Read(answer);
            return null;
        }
        return Saved(saved);
    }

    /// <summary>Reads what a save answered (<c>saved</c>).</summary>
    internal static SheetSave Saved(JsonObject saved)
    {
        var resin = saved["resin"] as JsonObject;
        return new(
            (bool)saved["changed"]! ? [.. saved["rows"]!.AsArray().Select(row => ResultsExport.DecodeRow(row!.AsObject()))] : null,
            (long)saved["next_key"]!, BoardEditor.Rekeyed(saved["rekeyed"]), (long?)saved["focus"],
            resin?["set"] is JsonObject set ? Resin(set) : null, (bool?)resin?["clear"] ?? false);
    }

    private static JsonObject Ask(JsonObject request) => BoardEditor.Parse(NativeEngine.RequirementEditor(request.ToJsonString()));

    private static (string Draft, SheetForm Form) Read(JsonObject answer) => ((string)answer["draft"]!, Sheet(answer["form"]!));

    /// <summary>Reads a FORM.</summary>
    internal static SheetForm Sheet(JsonNode form)
    {
        var origin = form["origin"]!;
        var effect = form["effect"]!;
        var stack = form["stack"]!;
        var resin = form["resin"]!;
        return new()
        {
            IsNew = (string?)form["mode"] == "new",
            Origin = (string?)origin["type"] switch { "row" => SheetOrigin.Row, "resin" => SheetOrigin.Resin, _ => SheetOrigin.New },
            OriginKey = (long?)origin["key"],
            Blanket = (bool)form["blanket"]!,
            InCluster = (bool)form["in_cluster"]!,
            ResinPicked = (bool)form["resin_picked"]!,
            Title = (string)form["title"]!,
            Preview = form["preview"] is JsonObject preview ? BoardEditor.Chip(preview) : null,
            Category = Choice(form["category"]!, Word),
            Kind = Choice(form["kind"]!, Word),
            WeaponType = Choice(form["weapon_type"]!, Word),
            Item = Choice(form["item"]!, value => (string?)value),
            Tier = ModeRange(form["tier"]!),
            Upgrade = ModeRange(form["upgrade"]!),
            Effect = new((bool)effect["visible"]!, (string)effect["label"]!, (string)effect["mode"]!, Options(effect["modes"], Word),
                (bool)effect["choices_visible"]!,
                [.. effect["choices"]!.AsArray().Select(choice => new SheetEffectChoice((string)choice!["value"]!, (string)choice["label"]!,
                    (string?)choice["group"] == "curse", (bool)choice["selected"]!))],
                Options(effect["groups"], Word), (string)effect["caption"]!),
            Uncursed = Toggle(form["uncursed"]!),
            Source = Choice(form["source"]!, value => (string?)value),
            FloorLimit = Floor(form["floor_limit"]!),
            ExcludeResin = Toggle(form["exclude_resin"]!),
            Transmutations = Stepper(form["transmutations"]!),
            SelectTrinket = Toggle(form["select_trinket"]!),
            Stack = new((bool)stack["visible"]!, (string)stack["label"]!, (int)stack["count"]!, (int)stack["min"]!, (int)stack["max"]!, (string)stack["value_label"]!,
                Floor(stack["copy_depth"]!), Stepper(stack["count_levels"]!)),
            Resin = new((bool)resin["visible"]!, (string)resin["label"]!, (bool)resin["auto"]!, Options(resin["modes"], value => (bool)value!),
                (string)resin["caption"]!, (double?)resin["amount"], (int)resin["min"]!, (int)resin["max"]!, Toggle(resin["include_mage_wand"]!)),
            Errors = BoardEditor.Strings(form["errors"]),
            CanSave = (bool)form["can_save"]!,
        };
    }

    private static string Word(JsonNode? value) => (string)value!;

    private static SheetChoice<T> Choice<T>(JsonNode control, Func<JsonNode?, T> value) =>
        new((bool)control["visible"]!, value(control["value"]), Options(control["options"], value));

    private static IReadOnlyList<SheetOption<T>> Options<T>(JsonNode? options, Func<JsonNode?, T> value) =>
        [.. (options as JsonArray ?? []).Select(option => new SheetOption<T>(value(option!["value"]), (string)option["label"]!,
            (string?)option["group"], (bool?)option["hidden"] ?? false))];

    private static SheetModeRange ModeRange(JsonNode control) => new((bool)control["visible"]!, (string)control["mode"]!,
        Options(control["modes"], Word), (bool)control["value_visible"]!, (int)control["value"]!, (int)control["min"]!, (int)control["max"]!,
        (string)control["value_label"]!);

    private static SheetToggle Toggle(JsonNode control) =>
        new((bool)control["visible"]!, (bool)control["value"]!, (string)control["label"]!, (string?)control["caption"]);

    private static SheetFloor Floor(JsonNode control) => new((bool)control["visible"]!, (bool)control["enabled"]!, (int)control["value"]!,
        Options(control["options"], value => (int)value!), (string)control["label"]!, (string)control["value_label"]!);

    private static SheetStepper Stepper(JsonNode control) => new((bool)control["visible"]!, (bool)control["enabled"]!, (int)control["value"]!,
        (int)control["min"]!, (int)control["max"]!, (string)control["label"]!, (string?)control["caption"], (bool)control["caption_visible"]!,
        (string)control["value_label"]!);

    /// <summary>A RESIN the editor saved.</summary>
    private static ResinCondition Resin(JsonObject resin)
    {
        var filter = resin["filter"] as JsonObject ?? new JsonObject();
        var auto = resin["amount"] is JsonValue amount && amount.TryGetValue(out string? word) && word == "auto";
        return new(auto, auto ? 0 : (int)resin["amount"]!, new ArcaneResinFilter(
            (bool?)filter["uncursed"] ?? true, (int?)filter["max_depth"], ResultsExport.SourceNamed((string?)filter["source"]),
            (bool?)filter["include_mage_wand"] ?? false));
    }
}
