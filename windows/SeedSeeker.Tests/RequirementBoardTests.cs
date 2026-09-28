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
/// saves, removals — come back through the real engine as the window adopts
/// them.
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
        foreach (var file in files)
        {
            var fixture = JsonNode.Parse(File.ReadAllText(file))!.AsObject();
            var request = RequestText(fixture["request"]!);
            var answer = (string?)fixture["envelope"] switch
            {
                "requirement_board" => NativeEngine.RequirementBoard(request),
                "requirement_editor" => NativeEngine.RequirementEditor(request),
                var other => throw new InvalidDataException($"{file}: unknown envelope {other}"),
            };
            Assert.True(JsonNode.DeepEquals(fixture["response"], JsonNode.Parse(answer)), $"{Path.GetFileName(file)} answered {answer}");
        }
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
        Assert.Empty(board.Problems);
        Assert.Null(board.Problem);

        // A ×3 stack of plain repeats: one chip, two hidden copies, a count badge.
        var rings = board.Entries[0];
        Assert.Equal([1L], rings.Members);
        Assert.Equal([2L, 3L], rings.Extras);
        Assert.Equal(new BoardBadge("×3", "×3", "3 of the same kind"), rings.CountBadge);
        Assert.Null(rings.TotalBadge);
        Assert.Equal(3, rings.Stack.Count);
        Assert.Equal(3, rings.Stack.CountMaximum);
        Assert.True(rings.Stack.CanCountLevels);
        Assert.Equal(11, rings.Stack.LevelCapacity);
        var ring = Assert.Single(rings.Chips);
        Assert.Equal("Ring of Might", ring.Name);
        Assert.Equal("ring_might", ring.Item);
        Assert.Equal(ItemKind.Ring, ring.Kind);
        Assert.Equal([new ChipTag("+2", Upgrade: true)], ring.Tags);
        Assert.Equal([new ChipRelation(RelationGlyph.Times, "3 of the same kind — the extra copies: any upgrade, any floor")], ring.Relations);
        Assert.Equal("Ring of Might\nexactly +2\n× 3 of the same kind — the extra copies: any upgrade, any floor", ring.Detail);
        Assert.Empty(ring.Join);
        Assert.All(ring.Refuse, refusal => Assert.Equal("mixed_category_stack", refusal.Reason));

        // A narrowed wildcard: its kind draws the sprite, its qualifiers are tags in order.
        var melee = Assert.Single(board.Entries[1].Chips);
        Assert.Equal(("Any melee", "Any Tier 3+ melee weapon", (string?)null, (ItemKind?)ItemKind.MeleeWeapon), (melee.Name, melee.Title, melee.Item, melee.Kind));
        Assert.Equal([new ChipTag("T3+"), new ChipTag("+2↑", true), new ChipTag("F≤9")], melee.Tags);
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
        Assert.Equal([new ChipTag("No resin")], cluster.Chips[1].TrailingTags);
        Assert.Equal("Any wand\nexactly +3 · excluded from Auto resin\nor Wand of Fireblast", cluster.Chips[1].Detail);
        Assert.Equal(board.Entries[2], board.EntryOf(6));
        Assert.Equal(cluster.Chips[1], board.ChipOf(6));
        // A hidden copy has no chip of its own.
        Assert.Null(board.ChipOf(2));
        Assert.Null(board.EntryOf(2));

        var armor = Assert.Single(board.Entries[4].Chips);
        Assert.True(board.Entries[4].Blanket);
        Assert.Equal(["Viscosity", "Brimstone"], armor.Effect!.Effects);
        Assert.Equal("effect: Viscosity/Brimstone", armor.Effect.Label);
        Assert.False(board.Entries[4].Stack.CanChangeCount);

        var resin = board.Resin!;
        Assert.Equal("Arcane Resin", resin.Name);
        Assert.Equal([new ChipTag("Auto"), new ChipTag("Mage +2")], resin.Tags);
        Assert.True(resin.Uncursed);
        Assert.Equal("Heap", resin.Tooltip);
        Assert.NotNull(resin.AmountTooltip);
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
        Assert.Equal(new BoardRefusal("mixed_category_stack", "Copies can only be grouped with the same item type."), refused.Refused);

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
                // The one row the catalog cannot read is the fixture's point.
                if ((string?)row!["item"] == "wand_of_wonders") continue;
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
        Assert.Equal(3, editor.View(query).EntryOf(KeyOf(query, "ring_might"))!.Stack.Count);

        // An edit that does nothing leaves the rows unwritten.
        var rows = query.Requirements;
        var same = Apply(editor, query, BoardEdit.SetCount(KeyOf(query, "ring_might"), 3));
        Assert.False(same.Changed);
        Assert.Null(same.Rows);
        Assert.Same(rows, query.Requirements);

        // The resin chip is part of the board, so a resin change asks again.
        query.ArcaneResin = 4;
        var withResin = editor.View(query);
        Assert.Equal([new ChipTag("≥4")], withResin.Resin!.Tags);
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
    public void JoinsRefusalsSavesAndRemovalsRunThroughTheEngine()
    {
        var editor = new BoardEditor();
        var query = Loaded(editor, Named("spear"), Named("mace"), new() { Kind = ItemKind.Wand });
        var spear = KeyOf(query, "spear"); var mace = KeyOf(query, "mace"); var wand = query.Requirements[2].Key;

        // A join makes one slot, the source placed after its target.
        var joined = Apply(editor, query, BoardEdit.Join(mace, spear));
        Assert.Equal(mace, joined.Focus);
        var cluster = Assert.Single(joined.View.Entries, item => item.Cluster is not null);
        Assert.Equal([spear, mace], cluster.Members);
        Assert.Contains("any_of", ResultsExport.EncodeQueryDocument(query));
        Assert.Equal(2, query.SlotCount);

        // A stacked cluster refuses a member of another category, and says why.
        Apply(editor, query, BoardEdit.SetCount(spear, 2));
        var board = editor.View(query);
        Assert.Equal(2, board.EntryOf(spear)!.Stack.Count);
        Assert.Contains(board.ChipOf(wand)!.Refuse, refusal => refusal.Key == spear);
        var drop = board.Drop(wand, DropKind.Cluster, spear);
        Assert.Equal(DropEffect.Refused, drop.Effect);
        var rows = query.Requirements;
        var refused = Apply(editor, query, BoardEdit.Join(wand, spear));
        Assert.Equal("mixed_category_stack", refused.Refused!.Reason);
        Assert.Equal(drop.Message, refused.Refused.Message);
        Assert.Same(rows, query.Requirements);

        // The board's save edit, which a sheet's save runs: a new chip is
        // appended with a fresh key; an unchanged save names its chip and
        // changes nothing.
        var added = Apply(editor, query, BoardEdit.Save(null, new() { Kind = ItemKind.Armor, UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 3 }, 2, null, 9));
        var armor = added.Focus!.Value;
        Assert.DoesNotContain(armor, rows.Select(row => row.Key));
        Assert.Equal(ItemKind.Armor, query.Requirements.Single(row => row.Key == armor).Kind);
        Assert.Equal(9, added.View.EntryOf(armor)!.Stack.CopyDepth);
        var saved = query.Requirements.Single(row => row.Key == armor).Clone();
        var unchanged = Apply(editor, query, BoardEdit.Save(armor, saved, 2, null, 9));
        Assert.False(unchanged.Changed);
        Assert.Equal(armor, unchanged.Focus);

        // A cluster member leaves on its own; removing a chip takes its copies.
        Apply(editor, query, BoardEdit.Detach(mace));
        Assert.Null(query.Requirements.Single(row => row.Key == mace).AlternativeGroup);
        var removed = Apply(editor, query, BoardEdit.Remove(armor));
        Assert.Null(removed.Focus);
        Assert.DoesNotContain(query.Requirements, row => row.Kind == ItemKind.Armor);
        Assert.Null(editor.View(query).Problem);
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
        var query = Loaded(editor);
        var longsword = Apply(editor, query, BoardEdit.Save(null, Named("longsword"), 3, null, 4)).Focus!.Value;
        Assert.Equal(
            "Longsword\nany upgrade\n× 3 of the same kind — the extra copies: any upgrade, floors 1–4",
            editor.View(query).ChipOf(longsword)!.Detail);
        // A combined level speaks for the upgrades, so the chip's own says nothing.
        var ring = Apply(editor, query, BoardEdit.Save(null, Named("ring_might"), 3, 5, null)).Focus!.Value;
        Assert.Equal("Ring of Might\nΣ up to 3 — levels add to ≥ 5", editor.View(query).ChipOf(ring)!.Detail);
        Assert.Equal(new BoardBadge("Σ ≥ 5", "Σ≥5", "Levels add to at least 5 (a +0 item counts 1)"), editor.View(query).EntryOf(ring)!.TotalBadge);
        Assert.Equal("≤3", editor.View(query).EntryOf(ring)!.CountBadge!.Text);
        // A cluster member names its peers.
        var spear = Apply(editor, query, BoardEdit.Save(null, Named("spear"), 1, null, null)).Focus!.Value;
        var shuriken = Apply(editor, query, BoardEdit.Save(null, Named("shuriken"), 1, null, null)).Focus!.Value;
        Apply(editor, query, BoardEdit.Join(shuriken, spear));
        Assert.Equal("Spear\nany upgrade\nor Shuriken", editor.View(query).ChipOf(spear)!.Detail);
        // v4.0.0's vault treasure reads as its own source, like every other one.
        var vault = Named("greatsword"); vault.Source = ScoutItemSource.VaultTreasure;
        var greatsword = Apply(editor, query, BoardEdit.Save(null, vault, 1, null, null)).Focus!.Value;
        Assert.Equal("Greatsword\nany upgrade · Vault treasure", editor.View(query).ChipOf(greatsword)!.Detail);
    }

    [Fact]
    public void ACopyAClusterFoldsAwaySpeaksThroughItsAnchorsChip()
    {
        // The window flags problems on chips alone: the anchor's chip carries
        // its hidden copies' problems, so a cluster's capsule never has to.
        var editor = new BoardEditor();
        var fireblast = Named("wand_fireblast"); fireblast.AlternativeGroup = 1; fireblast.IdentityGroup = 1;
        var query = Loaded(editor, fireblast, new() { Kind = ItemKind.Wand, AlternativeGroup = 1, IdentityGroup = 1 },
            new() { Kind = ItemKind.Wand, IdentityGroup = 1, MaximumDepth = 30 });
        var entry = Assert.Single(editor.View(query).Entries);
        Assert.Equal([query.Requirements[2].Key], entry.Extras);
        Assert.Equal("Requirement floor must be 1 through 24.", entry.Problem);
        Assert.Equal(entry.Problem, entry.Chips[0].Problem);
        Assert.Null(entry.Chips[1].Problem);
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
        Assert.Equal([new ChipTag("≥4"), new ChipTag("Mage +2"), new ChipTag("F≤9")], resin.Tags);
        Assert.Equal("Locked chest", resin.Tooltip);
        Assert.Null(resin.AmountTooltip);
        Assert.Equal("Arcane Resin\nat least 4 · starting Magic Missile contributes 2 resin · uncursed wands · Locked chest · floors 1–9", resin.Detail);
        // The amount and the Mage's wand are what the resin credits; the donors' floor is not.
        Assert.Equal(2, resin.CreditTags(query.ArcaneResinFilter.IncludeMageWand));

        query.ArcaneResinAuto = true;
        query.ArcaneResinFilter = query.ArcaneResinFilter with { IncludeMageWand = false, Source = null };
        resin = editor.View(query).Resin!;
        Assert.Equal([new ChipTag("Auto"), new ChipTag("F≤9")], resin.Tags);
        Assert.Null(resin.Tooltip);
        Assert.NotNull(resin.AmountTooltip);
        Assert.Equal(1, resin.CreditTags(query.ArcaneResinFilter.IncludeMageWand));
    }
}
