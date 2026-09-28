using System.Text.Json.Nodes;
using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The requirement board is the shared core's editor, reached through
/// <c>seedfinder_requirement_board</c>; the core's own tests own its rules.
/// These cover the bridge the window stands on: the bindings answer the
/// golden fixtures (crates/seedfinder-core/tests/fixtures/editor), the answer
/// reads into the typed board, rows cross the row codec unchanged, the board
/// is asked once per list, and the edits the window sends — joins, refusals,
/// counts, combined levels, removals — come back through the real engine as
/// the window adopts them. Stacks are per chip: a cluster member's badge and
/// count are its own.
/// </summary>
public sealed class RequirementBoardTests
{
    private static string FixtureDirectory() => Path.Combine(
        NativeEngineLibrary.WorkspaceRoot() ?? throw new InvalidOperationException("Could not locate the workspace root."),
        "crates", "seedfinder-core", "tests", "fixtures", "editor");

    private static JsonObject Fixture(string name) =>
        JsonNode.Parse(File.ReadAllText(Path.Combine(FixtureDirectory(), $"{name}.json")))!.AsObject();

    /// <summary>A request as a platform sends it: its JSON text, or the string a fixture pins verbatim.</summary>
    private static string RequestText(JsonNode request) =>
        request is JsonValue text && text.TryGetValue(out string? verbatim) ? verbatim : request.ToJsonString();

    private static ItemRequirement Named(string id, UpgradeMatch match = UpgradeMatch.Any, int upgrade = 0)
    {
        var item = ItemCatalog.Find(id)!;
        return new() { Kind = item.Kind, Item = item, UpgradeMatch = match, Upgrade = upgrade };
    }

    /// <summary>A query holding <paramref name="requirements"/>, taken in as a load would take it.</summary>
    private static QuerySettings Loaded(BoardEditor editor, params ItemRequirement[] requirements)
    {
        var query = new QuerySettings { Requirements = new(requirements) };
        editor.Load(query);
        return query;
    }

    /// <summary>Runs <paramref name="edits"/> and adopts the rows as the window does, only when they changed.</summary>
    private static BoardAnswer Apply(BoardEditor editor, QuerySettings query, params BoardEdit[] edits)
    {
        var answer = editor.Edit(query, edits);
        if (answer.Rows is { } rows) query.Requirements = new(rows);
        return answer;
    }

    private static long KeyOf(QuerySettings query, string id) => query.Requirements.First(requirement => requirement.Item?.Id == id).Key;

    [Fact]
    public void EveryGoldenFixtureIsAnsweredThroughTheBindings()
    {
        var files = Directory.GetFiles(FixtureDirectory(), "*.json");
        Assert.True(files.Length >= 30, "the editor fixtures moved");
        var read = 0;
        foreach (var file in files)
        {
            var fixture = JsonNode.Parse(File.ReadAllText(file))!.AsObject();
            var request = RequestText(fixture["request"]!);
            var board = (string?)fixture["envelope"] == "requirement_board";
            var answer = (string?)fixture["envelope"] switch
            {
                "requirement_board" => NativeEngine.RequirementBoard(request),
                "requirement_editor" => NativeEngine.RequirementEditor(request),
                var other => throw new InvalidDataException($"{file}: unknown envelope {other}"),
            };
            Assert.True(JsonNode.DeepEquals(fixture["response"], JsonNode.Parse(answer)), $"{Path.GetFileName(file)} answered {answer}");

            // Every answer reads into what the window draws from, hidden
            // controls included; one the editor could not read says so.
            if (fixture["response"]!["error"] is not null)
            {
                Assert.Throws<RequirementEditorException>(() => BoardEditor.Parse(answer));
                continue;
            }
            var failure = Record.Exception(() =>
            {
                if (board) { BoardEditor.Answer(answer); return; }
                var sheet = BoardEditor.Parse(answer);
                if (sheet["saved"] is JsonObject saved) { RequirementSheet.Saved(saved); return; }
                // Every numeric control holds its value within its range, and
                // every floor slider sits on one of its options.
                var form = RequirementSheet.Sheet(sheet["form"]!);
                foreach (var (value, min, max) in new[] {
                    (form.Tier.Value, form.Tier.Min, form.Tier.Max), (form.Upgrade.Value, form.Upgrade.Min, form.Upgrade.Max),
                    (form.Transmutations.Value, form.Transmutations.Min, form.Transmutations.Max), (form.Stack.Count, form.Stack.Min, form.Stack.Max),
                    (form.Stack.CountLevels.Value, form.Stack.CountLevels.Min, form.Stack.CountLevels.Max), (form.Resin.Min, 1, form.Resin.Max) })
                    Assert.InRange(value, min, max);
                Assert.True(form.FloorLimit.Selected >= 0 && form.Stack.CopyDepth.Selected >= 0);
            });
            Assert.True(failure is null, $"{Path.GetFileName(file)} did not read: {failure}");
            read++;
        }
        Assert.True(read >= 30, "the fixtures hold too few answers to read");
    }

    [Fact]
    public void TheBoardTourReadsIntoTypedEntries()
    {
        var answer = BoardEditor.Answer(Fixture("board-tour")["response"]!.ToJsonString());
        Assert.False(answer.Changed);
        Assert.Null(answer.Rows);
        Assert.Null(answer.Refused);
        Assert.Equal(9, answer.NextKey);
        var board = answer.View;
        Assert.Equal(new BoardCounts(4, 1), board.Counts);
        Assert.Equal(["r1", "r4", "c1", "r7", "r8"], board.Entries.Select(item => item.Id));
        // What a menu calls each entry: a chip's name, a cluster's members' names.
        Assert.Equal(["Ring of Might", "Any melee", "Wand of Fireblast or Any wand", "Rat Skull", "Any armor"], board.Entries.Select(item => item.Name));
        Assert.Empty(board.Problems);
        Assert.Null(board.Problem);

        // A ×3 stack of plain repeats: one chip with two hidden copies and its count badge.
        var rings = board.Entries[0];
        Assert.Equal([1L], rings.Members);
        Assert.Equal([2L, 3L], rings.Extras);
        var ring = Assert.Single(rings.Chips);
        Assert.Equal(new BoardBadge("×3", "×3", "3 of the same kind"), ring.CountBadge);
        Assert.Null(ring.TotalBadge);
        Assert.Equal([2L, 3L], ring.Copies);
        Assert.Equal(3, ring.Stack.Count);
        Assert.Equal(3, ring.Stack.CountMax);
        Assert.True(ring.Stack.CanCountLevels);
        Assert.Equal(11, ring.Stack.LevelCapacity);
        Assert.Equal("Ring of Might", ring.Name);
        Assert.Equal("ring_might", ring.Item);
        Assert.Equal(ItemKind.Ring, ring.Kind);
        Assert.Equal([new ChipTag("+2", TagStyle.Upgrade)], ring.Tags);
        Assert.Equal([new ChipRelation(RelationGlyph.Times, "3 of the same kind — the extra copies: any upgrade, any floor")], ring.Relations);
        Assert.Equal("Ring of Might\nexactly +2\n× 3 of the same kind — the extra copies: any upgrade, any floor", ring.Detail);
        // Every copy keeps its own chip's kind, so a stack joins any category (#190's refusal is lifted).
        Assert.Equal([4L, 5, 6, 7], ring.Join);
        Assert.Empty(ring.Refuse);

        // A narrowed wildcard: its kind draws the sprite, its qualifiers are tags in order.
        var melee = Assert.Single(board.Entries[1].Chips);
        Assert.Equal(("Any melee", "Any Tier 3+ melee weapon", (string?)null, (ItemKind?)ItemKind.MeleeWeapon), (melee.Name, melee.Title, melee.Item, melee.Kind));
        Assert.Equal([new ChipTag("T3+"), new ChipTag("+2↑", TagStyle.Upgrade), new ChipTag("F≤9")], melee.Tags);
        Assert.True(melee.Uncursed);
        Assert.True(melee.Effect!.AnyEnchantment);
        Assert.Equal("any enchantment", melee.Effect.Label);
        Assert.Equal(ItemCatalog.Enchantments, melee.Effect.Effects);
        Assert.Equal("Any Tier 3+ melee weapon, +2 or higher, any enchantment, uncursed, floors 1–9", melee.Description);

        // A cluster: every member a chip that may come out on its own, the resin exclusion after the effect cue.
        var cluster = board.Entries[2];
        Assert.Equal((int?)1, cluster.Cluster);
        Assert.Equal("Any of 2", cluster.Label);
        Assert.All(cluster.Chips, chip => Assert.True(chip.InCluster && chip.CanDetach));
        // Members without stacks of their own show no badge; the cluster has none to show.
        Assert.All(cluster.Chips, chip => Assert.Equal((null, null, 1), (chip.CountBadge, chip.TotalBadge, chip.Stack.Count)));
        Assert.Empty(cluster.Extras);
        Assert.Equal([new ChipTag("No resin")], cluster.Chips[1].TrailingTags);
        Assert.Equal("Any wand\nexactly +3 · excluded from Auto resin\nor Wand of Fireblast", cluster.Chips[1].Detail);
        Assert.Equal(board.Entries[2], board.EntryOf(6));
        Assert.Equal(cluster.Chips[1], board.ChipOf(6));
        // "Either/or with…" offers the cluster once, under its name, however many of its members a chip may join.
        Assert.Equal([1L, 5, 6, 7], board.ChipOf(4)!.Join);
        Assert.Equal([("Ring of Might", 1L), ("Wand of Fireblast or Any wand", 5L), ("Rat Skull", 7L)], board.JoinChoices(4));
        Assert.Equal([("Ring of Might", 1L), ("Any melee", 4L), ("Rat Skull", 7L)], board.JoinChoices(6));
        Assert.Equal([("Any melee", 4L), ("Wand of Fireblast or Any wand", 5L), ("Rat Skull", 7L)], board.JoinChoices(1));
        Assert.Empty(board.JoinChoices(2));
        // A hidden copy has no chip of its own.
        Assert.Null(board.ChipOf(2));
        Assert.Null(board.EntryOf(2));

        var armor = Assert.Single(board.Entries[4].Chips);
        Assert.True(board.Entries[4].Blanket);
        Assert.Equal(["Viscosity", "Brimstone"], armor.Effect!.Effects);
        Assert.Equal("effect: Viscosity/Brimstone", armor.Effect.Label);
        Assert.False(armor.Stack.CanChangeCount);
        Assert.Equal(1, armor.Stack.CountMax);

        var resin = board.Resin!;
        Assert.Equal("Arcane Resin", resin.Name);
        // The resin the chip counts is a credit, and each tag that needs it explains itself.
        Assert.Equal([
            new ChipTag("Auto", TagStyle.Credit, "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies"),
            new ChipTag("Mage +2", TagStyle.Credit, "Starting Magic Missile contributes 2 resin")], resin.Tags);
        Assert.True(resin.Uncursed);
        Assert.Equal("Heap", resin.Tooltip);
        Assert.Equal("Arcane Resin\nAuto · starting Magic Missile contributes 2 resin · uncursed wands · Heap", resin.Detail);
    }

    [Fact]
    public void EditAnswersCarryTheirRowsRefusalsAndProblems()
    {
        var saved = BoardEditor.Answer(Fixture("board-save-new")["response"]!.ToJsonString());
        Assert.True(saved.Changed);
        Assert.Equal(5, saved.Focus);
        Assert.Equal(7, saved.NextKey);
        Assert.Equal([1L, 5, 6], saved.Rows!.Select(row => row.Key));
        Assert.Equal([ItemKind.Wand, ItemKind.ThrownWeapon, ItemKind.Weapon], saved.Rows!.Select(row => row.Kind));
        Assert.Equal(new int?[] { null, 1, 1 }, saved.Rows!.Select(row => row.IdentityGroup));
        Assert.Equal(6, saved.Rows![2].MaximumDepth);

        var refused = BoardEditor.Answer(Fixture("board-join-refused")["response"]!.ToJsonString());
        Assert.False(refused.Changed);
        Assert.Null(refused.Rows);
        Assert.Equal(new BoardRefusal("no_free_group", "Every group label is in use. Remove a stack or a combined level first."), refused.Refused);

        var repaired = BoardEditor.Answer(Fixture("board-key-repair")["response"]!.ToJsonString());
        Assert.Equal([(0L, 5L), (4L, 6L)], repaired.Rekeyed);

        var problems = BoardEditor.Answer(Fixture("board-problems")["response"]!.ToJsonString()).View;
        Assert.Equal([ProblemScope.Row, ProblemScope.Row, ProblemScope.Group], problems.Problems.Select(problem => problem.Scope));
        Assert.Equal([1L, 3], problems.Problems[2].Keys);
        Assert.Equal("Requirement floor must be 1 through 24.", problems.Problem);
        Assert.Equal("Requirement floor must be 1 through 24.", problems.ChipOf(2)!.Problem);

        var blankets = BoardEditor.Answer(Fixture("board-problems-blankets-only")["response"]!.ToJsonString()).View;
        Assert.Equal(ProblemScope.List, blankets.Problems.Last().Scope);
        Assert.Empty(blankets.Problems.Last().Keys);
    }

    [Fact]
    public void AnUnreadableRequestIsReportedWithItsRow()
    {
        var error = Assert.Throws<RequirementEditorException>(() => BoardEditor.Answer(NativeEngine.RequirementBoard("{\"rows\": [")));
        Assert.StartsWith("invalid request:", error.Message);
        Assert.Null(error.Key);
        var row = Assert.Throws<RequirementEditorException>(() => BoardEditor.Answer("""{"error":"row 3 cannot be read","key":3}"""));
        Assert.Equal(3, row.Key);
    }

    [Fact]
    public void RowsCrossTheCodecInTheCoresCanonicalSpelling()
    {
        // Every row the core answered in a fixture reads back into the model
        // and writes out exactly as the core wrote it, so an adopted list
        // re-sent unchanged is the same list.
        var compared = 0;
        foreach (var file in Directory.GetFiles(FixtureDirectory(), "board-*.json"))
        {
            var response = JsonNode.Parse(File.ReadAllText(file))!["response"]!;
            foreach (var row in response["rows"] as JsonArray ?? [])
            {
                // The rows the catalog cannot read are those fixtures' point.
                if ((string?)row!["item"] is "wand_of_wonders" or "ring_of_wonders") continue;
                Assert.True(JsonNode.DeepEquals(row, ResultsExport.EncodeRow(ResultsExport.DecodeRow(row.AsObject()))), $"{Path.GetFileName(file)}: {row.ToJsonString()}");
                compared++;
            }
        }
        Assert.True(compared > 40);

        // Every field of the model, with the largest key every platform holds exactly.
        const long maxKey = (1L << 53) - 1;
        ItemRequirement[] rows = [
            new() { Key = maxKey, Kind = ItemKind.MeleeWeapon, Item = ItemCatalog.Find("sword"), UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 3,
                Effect = EffectFilter.OneOf(["Projecting", "Blocking"]), RequireUncursed = true, Source = ScoutItemSource.VaultTreasure, MaximumDepth = 19, AlternativeGroup = 7 },
            new() { Key = 2, Kind = ItemKind.ThrownWeapon, TierMatch = TierMatch.AtMost, Tier = 3, UpgradeMatch = UpgradeMatch.AtLeast, Upgrade = 2, Effect = EffectFilter.Enchantment(), IdentityGroup = 2 },
            new() { Key = 3, Kind = ItemKind.Ring, Item = ItemCatalog.Find("ring_might"), LevelSum = new(1, 4) },
            new() { Key = 4, Kind = ItemKind.Wand, ExcludeResin = true, Blanket = true },
            new() { Key = 5, Kind = ItemKind.Trinket, Item = ItemCatalog.Find("rat_skull"), TrinketTransmutations = 3 },
            new() { Key = 6, Kind = ItemKind.Trinket, Item = ItemCatalog.Find("mimic_tooth"), SelectTrinket = true },
            new() { Key = 7, Kind = ItemKind.Artifact, Item = ItemCatalog.Find("dried_rose"), ArtifactTransmutations = 4 },
            new() { Key = 8, Kind = ItemKind.Armor, TierMatch = TierMatch.Exactly, Tier = 4 },
        ];
        foreach (var row in rows)
        {
            var encoded = ResultsExport.EncodeRow(row);
            var decoded = ResultsExport.DecodeRow(encoded);
            Assert.True(JsonNode.DeepEquals(encoded, ResultsExport.EncodeRow(decoded)), encoded.ToJsonString());
            Assert.Equal(row.Key, decoded.Key);
            Assert.Equal(row.AlternativeGroup, decoded.AlternativeGroup);
        }
        Assert.Equal(maxKey, (long)ResultsExport.EncodeRow(rows[0])["key"]!);
        Assert.Equal(7, (int)ResultsExport.EncodeRow(rows[0])["alternative_group"]!);
        Assert.Equal(["Blocking", "Projecting"], ResultsExport.DecodeRow(ResultsExport.EncodeRow(rows[0])).Effect.Effects);
        Assert.True(ResultsExport.DecodeRow(ResultsExport.EncodeRow(rows[1])).Effect.AnyEnchantment);
        // A row the list has not keyed yet is the editor's to key.
        Assert.Equal(0, (long)ResultsExport.EncodeRow(new() { Key = -1, Kind = ItemKind.Wand })["key"]!);
    }

    [Fact]
    public void LoadingKeysTheListAfreshInItsCanonicalEncoding()
    {
        // Imports label every any_of entry, a lone member's too; the editor's
        // encoding leaves a lone alternative unlabelled.
        var query = ResultsExport.DecodeQueryDocument("""
            {"requirements":[{"any_of":[{"kind":"wand"}]},{"kind":"ring","item":"ring_might","upgrade":2},{"kind":"ring","item":"ring_might"}]}
            """);
        Assert.Equal(1, query.Requirements[0].AlternativeGroup);
        query.Requirements[1].Key = 1L << 60;
        var editor = new BoardEditor();
        editor.Load(query);
        Assert.Equal([1L, 2, 3], query.Requirements.Select(requirement => requirement.Key));
        Assert.Null(query.Requirements[0].AlternativeGroup);
        var board = editor.View(query);
        Assert.Equal(2, board.Entries.Count);
        Assert.Equal([3L], board.Entries[1].Extras);
        // Loading an already canonical list leaves its rows alone.
        var before = query.Requirements;
        editor.Load(query);
        Assert.Same(before, query.Requirements);
    }

    [Fact]
    public void LoadingMovesStackAndCombinedLevelLabelsIntoRange()
    {
        // A hand-written document may label a stack 7 or a combined level 9;
        // the portable formats stop at 4, so the list could not be searched or shared.
        var query = ResultsExport.DecodeQueryDocument("""
            {"requirements":[{"kind":"wand","upgrade":3,"identity_group":7},{"kind":"wand","identity_group":7},
            {"kind":"ring","item":"ring_might","level_sum":{"group":9,"at_least":3}},{"kind":"ring","item":"ring_might","level_sum":{"group":9,"at_least":3}}]}
            """);
        var editor = new BoardEditor();
        editor.Load(query);
        Assert.Equal(new int?[] { 1, 1, null, null }, query.Requirements.Select(requirement => requirement.IdentityGroup));
        Assert.Equal(new LevelSum?[] { null, null, new(1, 3), new(1, 3) }, query.Requirements.Select(requirement => requirement.LevelSum));
        Assert.Null(editor.Problem(query));
        Assert.NotNull(NativeEngine.TryEncodeShareLink(ResultsExport.EncodeQueryDocument(query)));
    }

    [Fact]
    public void EveryChipStepsItsOwnCount()
    {
        var editor = new BoardEditor();
        var spear = Named("spear"); spear.AlternativeGroup = 1; spear.IdentityGroup = 1;
        var query = Loaded(editor, Named("mace"), spear, new() { Kind = ItemKind.Ring, AlternativeGroup = 1 }, new() { Kind = ItemKind.Weapon, IdentityGroup = 1 });
        var board = editor.View(query);
        var maces = board.ChipOf(KeyOf(query, "mace"))!.Stack;
        Assert.True(maces.CanGrow);
        Assert.Equal((1, 3, 3), (maces.Count, maces.CountMax, maces.Max));
        // A label on one member is that member's own stack — two Spears, or
        // any ring — and its copy keeps the Spear's kind, so a cluster
        // spanning categories grows like any chip.
        var spears = board.ChipOf(KeyOf(query, "spear"))!;
        var ring = board.EntryOf(spears.Key)!.Chips[1];
        Assert.Equal(("×2", 2, 3, true), (spears.CountBadge!.Text, spears.Stack.Count, spears.Stack.CountMax, spears.Stack.CanGrow));
        Assert.Equal([query.Requirements[3].Key], spears.Copies);
        Assert.Equal(((BoardBadge?)null, 1, 3), (ring.CountBadge, ring.Stack.Count, ring.Stack.CountMax));
        Assert.False(spears.Stack.CanCountLevels);

        // The other member grows beside it under a label of its own; the Spears keep theirs.
        var grown = Apply(editor, query, BoardEdit.SetCount(ring.Key, 2));
        Assert.Equal(ring.Key, grown.Focus);
        Assert.Equal(["×2", "×2"], grown.View.EntryOf(ring.Key)!.Chips.Select(chip => chip.CountBadge!.Text));
        var labels = query.Requirements.Where(row => row.AlternativeGroup is not null).Select(row => row.IdentityGroup).ToList();
        Assert.Equal(2, labels.OfType<int>().Distinct().Count());
    }

    [Fact]
    public void MembersSharingALabelEachShowTheStack()
    {
        // One label on Frost and Disintegration: two of the same wand,
        // whichever matched. Each shows ×2 over the one copy they share;
        // Lightning, unlabelled, shows nothing, and neither does the cluster.
        var board = BoardEditor.Answer(Fixture("board-cluster-alike-stacks")["response"]!.ToJsonString()).View;
        var cluster = Assert.Single(board.Entries);
        Assert.Equal([4L], cluster.Extras);
        Assert.Equal(new string?[] { "×2", "×2", null }, cluster.Chips.Select(chip => chip.CountBadge?.Text));
        Assert.Equal([[4L], [4L], []], cluster.Chips.Select(chip => chip.Copies.ToArray()));
    }

    [Fact]
    public void TheBoardIsAskedOncePerRequirementList()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, Named("ring_might", UpgradeMatch.Exactly, 2), new() { Kind = ItemKind.Wand });
        var asked = editor.Requests;
        var board = editor.View(query);
        Assert.Same(board, editor.View(query));
        // Settings the board does not draw leave it alone.
        query.MaximumDepth = 12; query.AutoApplyTrinket = !query.AutoApplyTrinket;
        Assert.Same(board, editor.View(query));
        Assert.Equal(asked, editor.Requests);

        // An edit's answer is the board of the list it leaves.
        var grown = Apply(editor, query, BoardEdit.SetCount(KeyOf(query, "ring_might"), 3));
        Assert.True(grown.Changed);
        Assert.Same(grown.View, editor.View(query));
        Assert.Equal(asked + 1, editor.Requests);
        Assert.Equal(3, editor.View(query).ChipOf(KeyOf(query, "ring_might"))!.Stack.Count);

        // An edit that does nothing leaves the rows unwritten.
        var rows = query.Requirements;
        var same = Apply(editor, query, BoardEdit.SetCount(KeyOf(query, "ring_might"), 3));
        Assert.False(same.Changed);
        Assert.Null(same.Rows);
        Assert.Same(rows, query.Requirements);

        // The resin chip is part of the board, so a resin change asks again.
        query.ArcaneResin = 4;
        var withResin = editor.View(query);
        Assert.Equal([new ChipTag("≥4", TagStyle.Credit)], withResin.Resin!.Tags);
        Assert.Equal(asked + 3, editor.Requests);
    }

    [Fact]
    public void AListThatWasNeverLoadedIsKeyedByTheEditor()
    {
        var editor = new BoardEditor();
        var query = new QuerySettings { Requirements = [new() { Kind = ItemKind.Wand }, new() { Kind = ItemKind.Ring }] };
        var board = editor.View(query);
        Assert.Equal(query.Requirements.Select(requirement => requirement.Key), board.Entries.SelectMany(item => item.Members));
        Assert.All(query.Requirements, requirement => Assert.NotEqual(0, requirement.Key));
    }

    [Fact]
    public void JoinsRefusalsDetachesAndRemovalsRunThroughTheEngine()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, Named("spear"), Named("mace"), new() { Kind = ItemKind.Wand }, new() { Kind = ItemKind.Armor, UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 3 });
        var spear = KeyOf(query, "spear"); var mace = KeyOf(query, "mace"); var wand = query.Requirements[2].Key; var armor = query.Requirements[3].Key;

        // A join makes one slot, the source placed after its target.
        var joined = Apply(editor, query, BoardEdit.Join(mace, spear));
        Assert.Equal(mace, joined.Focus);
        var cluster = Assert.Single(joined.View.Entries, item => item.Cluster is not null);
        Assert.Equal([spear, mace], cluster.Members);
        Assert.Contains("any_of", ResultsExport.EncodeQueryDocument(query));
        Assert.Equal(3, query.SlotCount);

        // A member's stack is its own, and a cluster holding one takes a
        // member of another category: every copy keeps its own chip's kind
        // (#190 refused this join).
        Apply(editor, query, BoardEdit.SetCount(spear, 2));
        var board = editor.View(query);
        Assert.Equal((2, 1), (board.ChipOf(spear)!.Stack.Count, board.ChipOf(mace)!.Stack.Count));
        Assert.Equal(DropEffect.Join, board.Drop(wand, DropKind.Cluster, spear).Effect);
        Apply(editor, query, BoardEdit.Join(wand, spear));
        board = editor.View(query);
        Assert.Equal([spear, mace, wand], board.EntryOf(spear)!.Members);
        Assert.Equal(["×2", null, null], board.EntryOf(spear)!.Chips.Select(chip => chip.CountBadge?.Text));

        // A cluster member leaves on its own; removing a chip takes its copies.
        Apply(editor, query, BoardEdit.Detach(mace));
        Assert.Null(query.Requirements.Single(row => row.Key == mace).AlternativeGroup);
        Apply(editor, query, BoardEdit.SetCount(armor, 2));
        Assert.Equal(2, query.Requirements.Count(row => row.Kind == ItemKind.Armor));
        var removed = Apply(editor, query, BoardEdit.Remove(armor));
        Assert.Null(removed.Focus);
        Assert.DoesNotContain(query.Requirements, row => row.Kind == ItemKind.Armor);
        Assert.Null(editor.View(query).Problem);
    }

    [Fact]
    public void AJoinWithNoLabelFreeIsRefusedAndSaysWhy()
    {
        // Four stacks hold every label; Disintegration onto Frost ×2 would
        // need a fifth for the Frosts, which stay a stack as a member.
        var editor = new BoardEditor();
        var request = Fixture("board-join-refused")["request"]!;
        var query = new QuerySettings { Requirements = new(request["rows"]!.AsArray().Select(row => ResultsExport.DecodeRow(row!.AsObject()))) };
        var board = editor.View(query);
        Assert.Contains(board.ChipOf(11)!.Refuse, refusal => refusal.Key == 9 && refusal.Reason == "no_free_group");
        var drop = board.Drop(11, DropKind.Chip, 9);
        Assert.Equal(DropEffect.Refused, drop.Effect);
        var rows = query.Requirements;
        var refused = Apply(editor, query, BoardEdit.Join(11, 9));
        Assert.Equal("no_free_group", refused.Refused!.Reason);
        Assert.Equal(drop.Message, refused.Refused.Message);
        Assert.Same(rows, query.Requirements);
    }

    [Fact]
    public void CombinedLevelsAreCountedAndClearedThroughTheEngine()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, Named("ring_might", UpgradeMatch.Exactly, 2));
        var ring = KeyOf(query, "ring_might");
        Apply(editor, query, BoardEdit.SetCount(ring, 2));

        // "Count levels together" starts at the item count, within the rings' capacity.
        var counting = Apply(editor, query, BoardEdit.ToggleLevels(ring));
        Assert.True(counting.Changed);
        var stack = counting.View.ChipOf(ring)!.Stack;
        Assert.Equal((2, (int?)2, "≤2"), (stack.DefaultTotal, stack.Total, stack.CountText));
        Assert.Equal(new LevelSum(1, 2), query.Requirements[0].LevelSum);

        // The Σ badge's flyout sets the total, and clearing it stops counting.
        var total = Apply(editor, query, BoardEdit.SetTotal(ring, 5));
        Assert.Equal(5, total.View.ChipOf(ring)!.Stack.Total);
        Assert.Equal(new BoardBadge("Σ ≥ 5", "Σ≥5", "Levels add to at least 5 (a +0 item counts 1)"), total.View.ChipOf(ring)!.TotalBadge);
        Assert.All(query.Requirements, row => Assert.Equal(new LevelSum(1, 5), row.LevelSum));
        var cleared = Apply(editor, query, BoardEdit.SetTotal(ring, null));
        Assert.True(cleared.Changed);
        var chip = cleared.View.ChipOf(ring)!;
        Assert.Null(chip.Stack.Total);
        Assert.Null(chip.TotalBadge);
        Assert.Equal("×2", chip.CountBadge!.Text);
        Assert.All(query.Requirements, row => Assert.Null(row.LevelSum));

        // Turned on and off again, the list is as it was, and nothing is written back.
        var rows = query.Requirements;
        var twice = Apply(editor, query, BoardEdit.ToggleLevels(ring), BoardEdit.ToggleLevels(ring));
        Assert.False(twice.Changed);
        Assert.Same(rows, query.Requirements);
    }

    [Fact]
    public void ADropIsDecidedFromTheDraggedChip()
    {
        var editor = new BoardEditor();
        var ordinary = Named("spear"); var alternative = Named("mace"); ordinary.AlternativeGroup = alternative.AlternativeGroup = 1;
        var blanket = new ItemRequirement { Kind = ItemKind.Wand, Blanket = true };
        var query = Loaded(editor, ordinary, alternative, new() { Kind = ItemKind.Armor }, blanket);
        var board = editor.View(query);
        var (spear, mace, armor, wand) = (query.Requirements[0].Key, query.Requirements[1].Key, query.Requirements[2].Key, query.Requirements[3].Key);

        Assert.Equal(DropEffect.Join, board.Drop(armor, DropKind.Chip, spear).Effect);
        Assert.Equal(DropEffect.Join, board.Drop(armor, DropKind.Cluster, spear).Effect);
        // A chip's own cluster, and the other section, change nothing.
        Assert.Equal(DropEffect.None, board.Drop(mace, DropKind.Cluster, spear).Effect);
        Assert.Equal(DropEffect.None, board.Drop(wand, DropKind.Chip, armor).Effect);
        // The empty board detaches a cluster member of its own section; a lone chip stays.
        Assert.Equal(DropEffect.Detach, board.Drop(mace, DropKind.Board).Effect);
        Assert.Equal(DropEffect.None, board.Drop(mace, DropKind.Board, blanketBoard: true).Effect);
        Assert.Equal(DropEffect.None, board.Drop(armor, DropKind.Board).Effect);
        Assert.Equal(DropEffect.Remove, board.Drop(wand, DropKind.Remove).Effect);
        Assert.Equal(DropEffect.None, board.Drop(404, DropKind.Remove).Effect);
    }

    [Fact]
    public void TheStartGateAsksTheQueryFirstThenTheBoard()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, new ItemRequirement { Kind = ItemKind.Wand, Blanket = true });
        Assert.Equal("Add at least one ordinary requirement.", editor.Problem(query));
        query.ArcaneResin = 70_000;
        Assert.Equal("Arcane Resin must be 0..65535, with a valid wand floor and source.", editor.Problem(query));
        // The resin chip still draws while the amount is out of range.
        Assert.NotNull(editor.View(query).Resin);
        // A requirement's own problem is the board's wording, and its chip's.
        query = Loaded(editor, new ItemRequirement { Kind = ItemKind.Artifact });
        Assert.Equal("Select an artifact.", editor.Problem(query));
        Assert.Equal(editor.Problem(query), editor.View(query).Entries[0].Chips[0].Problem);
    }

    [Fact]
    public void TheChipDetailReadsTheStackAndTheRelationsAroundIt()
    {
        var editor = new BoardEditor();
        var copy = Named("longsword"); copy.MaximumDepth = 4;
        // v4.0.0's vault treasure reads as its own source, like every other one.
        var vault = Named("greatsword"); vault.Source = ScoutItemSource.VaultTreasure;
        var query = Loaded(editor, Named("longsword"), copy, copy.Clone(), Named("ring_might"), Named("spear"), Named("shuriken"), vault);
        var (longsword, ring, spear, shuriken) = (KeyOf(query, "longsword"), KeyOf(query, "ring_might"), KeyOf(query, "spear"), KeyOf(query, "shuriken"));
        Assert.Equal(
            "Longsword\nany upgrade\n× 3 of the same kind — the extra copies: any upgrade, floors 1–4",
            editor.View(query).ChipOf(longsword)!.Detail);
        Assert.Equal("Greatsword\nany upgrade · Vault treasure", editor.View(query).ChipOf(KeyOf(query, "greatsword"))!.Detail);
        // A combined level speaks for the upgrades, so the chip's own says nothing.
        Apply(editor, query, BoardEdit.SetCount(ring, 3), BoardEdit.SetTotal(ring, 5));
        Assert.Equal("Ring of Might\nΣ up to 3 — levels add to ≥ 5", editor.View(query).ChipOf(ring)!.Detail);
        Assert.Equal("≤3", editor.View(query).ChipOf(ring)!.CountBadge!.Text);
        // A cluster member names its peers.
        Apply(editor, query, BoardEdit.Join(shuriken, spear));
        Assert.Equal("Spear\nany upgrade\nor Shuriken", editor.View(query).ChipOf(spear)!.Detail);
    }

    [Fact]
    public void ACopyAClusterFoldsAwaySpeaksThroughTheChipsItBelongsTo()
    {
        // The window flags problems on chips alone: a chip carries its own
        // hidden copies' problems, so a cluster's capsule never has to.
        var editor = new BoardEditor();
        var fireblast = Named("wand_fireblast"); fireblast.AlternativeGroup = 1; fireblast.IdentityGroup = 1;
        var copy = new ItemRequirement { Kind = ItemKind.Wand, IdentityGroup = 1, MaximumDepth = 30 };
        // Both members carry the label: the copy is both chips', each drawn ×2.
        var query = Loaded(editor, fireblast, new() { Kind = ItemKind.Wand, AlternativeGroup = 1, IdentityGroup = 1 }, copy);
        var entry = Assert.Single(editor.View(query).Entries);
        Assert.Equal([query.Requirements[2].Key], entry.Extras);
        Assert.Equal("Requirement floor must be 1 through 24.", entry.Problem);
        Assert.All(entry.Chips, chip => Assert.Equal((entry.Problem, "×2"), (chip.Problem, chip.CountBadge!.Text)));
        // Fireblast alone carries it: the copy is Fireblast's, and only its chip is flagged.
        query = Loaded(editor, fireblast.Clone(), new() { Kind = ItemKind.Wand, AlternativeGroup = 1 }, copy.Clone());
        entry = Assert.Single(editor.View(query).Entries);
        Assert.Equal(entry.Problem, entry.Chips[0].Problem);
        Assert.Equal([query.Requirements[2].Key], entry.Chips[0].Copies);
        Assert.Null(entry.Chips[1].Problem);
        Assert.Null(entry.Chips[1].CountBadge);
    }

    [Fact]
    public void TheResinChipDrawsTheQuerysResin()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, new ItemRequirement { Kind = ItemKind.Wand });
        Assert.Null(editor.View(query).Resin);
        query.ArcaneResin = 4;
        query.ArcaneResinFilter = new(true, 9, ScoutItemSource.LockedChest, true);
        var resin = editor.View(query).Resin!;
        // The amount and the Mage's wand are what the resin counts, tinted
        // apart from the donors' floor; a fixed amount needs no explaining.
        Assert.Equal([
            new ChipTag("≥4", TagStyle.Credit),
            new ChipTag("Mage +2", TagStyle.Credit, "Starting Magic Missile contributes 2 resin"),
            new ChipTag("F≤9")], resin.Tags);
        Assert.Equal("Locked chest", resin.Tooltip);
        Assert.Equal("Arcane Resin\nat least 4 · starting Magic Missile contributes 2 resin · uncursed wands · Locked chest · floors 1–9", resin.Detail);
        // The same chip as the fixture pins for this resin.
        var pinned = BoardEditor.Answer(Fixture("board-resin-credit")["response"]!.ToJsonString()).View.Resin!;
        Assert.Equal(pinned.Tags, resin.Tags);
        Assert.Equal((pinned.Tooltip, pinned.Description), (resin.Tooltip, resin.Description));
        Assert.Equal(pinned.Details, resin.Details);

        query.ArcaneResinAuto = true;
        query.ArcaneResinFilter = query.ArcaneResinFilter with { IncludeMageWand = false, Source = null };
        resin = editor.View(query).Resin!;
        Assert.Equal([
            new ChipTag("Auto", TagStyle.Credit, "Enough resin to upgrade kept wands to +3, excluding No resin wands and reforge copies"),
            new ChipTag("F≤9")], resin.Tags);
        Assert.Null(resin.Tooltip);
    }
}
