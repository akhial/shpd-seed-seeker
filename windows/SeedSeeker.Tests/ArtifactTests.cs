using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class ArtifactTests
{
    [Fact]
    public void ScoutArtifactsShowRoundedGameLevels()
    {
        foreach (var item in ItemCatalog.All.Where(item => item.Kind == ItemKind.Artifact))
        {
            var expected = item.Id switch { "sandals_of_nature" => 7,
                "ethereal_chains" or "timekeepers_hourglass" => 6, _ => 5 };
            var scout = new ScoutItem(item, 19, 5, null, false, ScoutItemSource.ImpReward, 0, 0, 0);
            Assert.Equal(expected, scout.DisplayedUpgrade);
            Assert.Equal(5, scout.Upgrade);
            Assert.Equal(0, (scout with { Upgrade = 0 }).DisplayedUpgrade);
        }
    }

    private static ItemRequirement Sandals() => new()
    {
        Kind = ItemKind.Artifact, Item = ItemCatalog.Find("sandals_of_nature"),
        UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 5,
        Source = ScoutItemSource.ImpReward, MaximumDepth = 19, RequireUncursed = true,
    };

    [Fact]
    public void ArtifactCatalogAndEditorBoundsMatchNamedArtifacts()
    {
        var items = ItemCatalog.All.Where(item => item.Kind == ItemKind.Artifact).ToList();
        Assert.Equal(11, items.Count);
        Assert.Equal(11, items.Select(item => item.Id).Distinct().Count());
        Assert.All(items, item => Assert.Null(item.Tier));
        Assert.Empty(ItemCatalog.Modifiers(ItemKind.Artifact));
        var query = new QuerySettings { Requirements = [Sandals()] };
        var chip = new BoardEditor().View(query).Entries.Single().Chips.Single();
        Assert.Equal(["exactly +5", "uncursed", "Imp reward", "floors 1–19"], chip.Details);
        // The sheet always names one of them, never stacks one, and allows up to ten transmutations.
        var sheet = RequirementSheet.Open(query, chip.Key).Form;
        Assert.Equal(items.Select(item => item.Id).Order(), sheet.Item.Options.Select(option => option.Value!).Order());
        Assert.Equal("sandals_of_nature", sheet.Item.Value);
        Assert.False(sheet.Stack.Visible);
        Assert.True(sheet.Transmutations.Visible);
        Assert.Equal(10, sheet.Transmutations.Max);
    }

    [Fact]
    public void AnArtifactsUpgradeSurvivesItsSheet()
    {
        // No control shows an artifact's upgrade; the sheet keeps it all the same.
        var query = new QuerySettings { Requirements = [Sandals()] };
        new BoardEditor().Load(query);
        var key = query.Requirements[0].Key;
        var untouched = RequirementSheet.Open(query, key).Save(query)!;
        Assert.Null(untouched.Rows);
        Assert.False(untouched.ApplyTo(query));

        var sheet = RequirementSheet.Open(query, key);
        Assert.False(sheet.Form.Upgrade.Visible);
        sheet.Change(SheetChange.SetTransmutationsEnabled(true));
        var saved = sheet.Save(query)!;
        Assert.True(saved.ApplyTo(query));
        var sandals = Assert.Single(query.Requirements);
        Assert.Equal((UpgradeMatch.Exactly, 5, 1), (sandals.UpgradeMatch, sandals.Upgrade, sandals.ArtifactTransmutations));
        Assert.Equal((ScoutItemSource.ImpReward, (int?)19, true), (sandals.Source, sandals.MaximumDepth, sandals.RequireUncursed));
    }

    [Fact]
    public void ArtifactsCannotBeWildcardsOrStacks()
    {
        var editor = new BoardEditor();
        var unnamed = new QuerySettings { Requirements = [new() { Kind = ItemKind.Artifact }] };
        Assert.Equal("Select an artifact.", editor.Problem(unnamed));
        Assert.Null(NativeEngine.TryEncodeShareLink(ResultsExport.EncodeQueryDocument(unnamed)));
        var query = new QuerySettings { Requirements = [Sandals()] };
        editor.Load(query);
        var item = Assert.Single(editor.View(query).Entries);
        Assert.False(item.Stack.CanGrow);
        Assert.False(item.Stack.CanChangeCount);
        Assert.False(editor.Edit(query, BoardEdit.SetCount(item.Members[0], 3)).Changed);
        // The same artifact twice is two finds, never a stack of copies.
        var plain = new ItemRequirement { Kind = ItemKind.Artifact, Item = ItemCatalog.Find("dried_rose") };
        var repeated = new QuerySettings { Requirements = [plain, plain.Clone(), Sandals()] };
        editor.Load(repeated);
        Assert.Equal(3, editor.View(repeated).Entries.Count);
        var joined = editor.Edit(repeated, BoardEdit.Join(repeated.Requirements[0].Key, repeated.Requirements[2].Key));
        Assert.All(joined.Rows!, requirement => Assert.Null(requirement.IdentityGroup));
    }

    [Fact]
    public void ArtifactConstraintsAndAlternativesSurviveDocumentsLinksAndSettings()
    {
        var alternative = new ItemRequirement { Kind = ItemKind.Artifact, Item = ItemCatalog.Find("dried_rose"), MaximumDepth = 9 };
        var query = new QuerySettings { Requirements = [Sandals(), alternative] };
        var editor = new BoardEditor();
        editor.Load(query);
        query.Requirements = new(editor.Edit(query, BoardEdit.Join(query.Requirements[1].Key, query.Requirements[0].Key)).Rows!);
        var document = ResultsExport.EncodeQueryDocument(query);
        Assert.Contains("\"kind\":\"artifact\"", document);
        var link = NativeEngine.TryEncodeShareLink(document);
        Assert.NotNull(link);
        var linkDocument = NativeEngine.TryDecodeShareText(link!);
        Assert.NotNull(linkDocument);
        var restored = ResultsExport.DecodeQueryDocument(linkDocument!);
        Assert.Equal(1, QueryRelationships.SlotCount(restored.Requirements));
        Assert.All(restored.Requirements, requirement => Assert.Equal(ItemKind.Artifact, requirement.Kind));
        var sandals = restored.Requirements.Single(requirement => requirement.Item?.Id == "sandals_of_nature");
        Assert.Equal(5, sandals.Upgrade);
        Assert.Equal(19, sandals.MaximumDepth);
        Assert.True(sandals.RequireUncursed);
        Assert.Equal(ScoutItemSource.ImpReward, sandals.Source);
        var file = ResultsExport.Encode(query, ["AAA-AAA-AAA"], "test");
        Assert.Equal(document, ResultsExport.EncodeQueryDocument(ResultsExport.Decode(file).Query));
        var settings = JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!;
        Assert.Equal(document, ResultsExport.EncodeQueryDocument(settings));
    }

    [Fact]
    public void NativeScoutKeepsArtifactUpgradesAndMatchIndices()
    {
        var world = new NativeEngine().Scout("AAA-AAA-AAA", 0);
        Assert.Equal(4, world.Items.Count(item => item.Item.Kind == ItemKind.Artifact));
        var sandals = Assert.Single(world.Items, item => item.Item.Id == "sandals_of_nature");
        Assert.Equal(5, sandals.Upgrade);
        Assert.Equal(19, sandals.Depth);
        Assert.Equal(ScoutItemSource.ImpReward, sandals.Source);
        var query = new QuerySettings { Requirements = [Sandals()] };
        var matches = NativeEngine.ScoutMatches(world.Seed, 0, query);
        Assert.Equal(1, matches.MatchedRequirements);
        Assert.Equal(sandals, world.Items[Assert.Single(matches.Matched)]);
        query.Requirements[0].MaximumDepth = 18;
        Assert.Empty(NativeEngine.ScoutMatches(world.Seed, 0, query).Matched);
    }

    [Theory]
    [InlineData(double.NaN)]
    [InlineData(double.PositiveInfinity)]
    public void UnavailableProbabilityHasAStableDisplay(double probability)
    {
        var status = new SearchStatus(SearchState.Running, 1, 10, 0, probability);
        Assert.True(status.ProbabilityUnavailable);
        Assert.Equal("unavailable", status.ProbabilityDescription);
    }
}
