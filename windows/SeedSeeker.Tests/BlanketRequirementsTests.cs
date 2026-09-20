using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class BlanketRequirementsTests
{
    private static ItemRequirement Wand(bool blanket = false) => new()
    {
        Kind = ItemKind.Wand, Item = ItemCatalog.Find("wand_frost"), Blanket = blanket,
    };

    [Fact]
    public void BlanketsSurviveDocumentsLinksResultsAndSettings()
    {
        var query = new QuerySettings { Requirements = new(new[] { "wand_lightning", "wand_disintegration", "wand_frost" }
            .Select(id => new ItemRequirement { Kind = ItemKind.Wand, Item = ItemCatalog.Find(id), Upgrade = 2, UpgradeMatch = UpgradeMatch.AtLeast })) };
        query.Requirements.Add(new() { Kind = ItemKind.Wand, Blanket = true, Upgrade = 3,
            UpgradeMatch = UpgradeMatch.Exactly, Source = ScoutItemSource.WandmakerReward });
        var document = ResultsExport.EncodeQueryDocument(query);
        Assert.Contains("\"blanket\":true", document);
        var link = NativeEngine.TryEncodeShareLink(document);
        Assert.NotNull(link);
        foreach (var restored in new[] {
            ResultsExport.DecodeQueryDocument(document),
            ResultsExport.DecodeQueryDocument(NativeEngine.TryDecodeShareText(link!)!),
            ResultsExport.Decode(ResultsExport.Encode(query, ["AAA-AAA-AAA"], "test")).Query,
            JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!, query.Clone(),
        })
        {
            Assert.Equal(new[] { false, false, false, true }, restored.Requirements.Select(r => r.Blanket));
            Assert.Equal(3, restored.Requirements.Last().Upgrade);
            Assert.Equal(ScoutItemSource.WandmakerReward, restored.Requirements.Last().Source);
            Assert.Null(QueryRelationships.Validate(restored));
        }
    }

    [Fact]
    public void BlanketsStaySeparateFromStacksAndOrdinaryAlternatives()
    {
        var requirements = new[] { Wand(), Wand(true), Wand(true) };
        var board = QueryRelationships.BoardItems(requirements);
        Assert.Equal(3, board.Count);
        Assert.False(QueryRelationships.CanStack(requirements, board[1]));
        Assert.Equal(3, QueryRelationships.SetStackCount(requirements, board[1], 3).Count);
        Assert.Equal(requirements, QueryRelationships.JoinAlternatives(requirements, 0, 1));
        var grouped = QueryRelationships.JoinAlternatives(requirements, 1, 2);
        Assert.Equal(2, QueryRelationships.BoardCount(grouped));
        Assert.All(grouped, r => Assert.Null(r.IdentityGroup));
        var query = new QuerySettings { Requirements = new(grouped) };
        Assert.Null(QueryRelationships.Validate(query));
        Assert.NotNull(NativeEngine.TryEncodeShareLink(ResultsExport.EncodeQueryDocument(query)));
        Assert.Equal(3, QueryRelationships.BoardCount(QueryRelationships.Detach(grouped, 1)));
    }

    [Fact]
    public void InvalidBlanketsAreRejectedBeforeSearching()
    {
        Assert.NotNull(QueryRelationships.Validate(new() { Requirements = [Wand(true)] }));
        var ordinary = Wand(); ordinary.AlternativeGroup = 1;
        var blanket = Wand(true); blanket.AlternativeGroup = 1;
        Assert.NotNull(QueryRelationships.Validate(new() { Requirements = [ordinary, blanket] }));
        blanket.AlternativeGroup = null; blanket.IdentityGroup = 1;
        Assert.NotNull(QueryRelationships.Validate(new() { Requirements = [Wand(), blanket] }));
        blanket.IdentityGroup = null; blanket.SelectTrinket = true;
        Assert.NotNull(QueryRelationships.Validate(new() { Requirements = [Wand(), blanket] }));
    }

    [Fact]
    public void ConflictingBlanketStopsBeforeScanningAndReportsImpossible()
    {
        var query = new QuerySettings { Requirements = new(new[] { "wand_lightning", "wand_disintegration" }
            .Select(id => new ItemRequirement { Kind = ItemKind.Wand, Item = ItemCatalog.Find(id), Upgrade = 2, UpgradeMatch = UpgradeMatch.Exactly })) };
        query.Requirements.Add(new() { Kind = ItemKind.Wand, Blanket = true, Upgrade = 3,
            UpgradeMatch = UpgradeMatch.Exactly });
        using var search = new NativeEngine().StartResumed(query, 42, 1_000, 1);
        Assert.True(SpinWait.SpinUntil(() => search.Status().State != SearchState.Running,
            TimeSpan.FromSeconds(5)));
        var status = search.Status();
        Assert.True(status.IsImpossibleQuery);
        Assert.Equal(0, status.Scanned);
        Assert.Empty(search.Poll(1));
        Assert.Equal((42L, 1_000L), search.ResumeHint());
        Assert.False(new SearchStatus(SearchState.Completed, 0, 0, 0, 0).IsImpossibleQuery);
        Assert.False(new SearchStatus(SearchState.Cancelled, 0, 1_000, 0, 0).IsImpossibleQuery);
        Assert.False(new SearchStatus(SearchState.Completed, 1_000, 1_000, 0, 0).IsImpossibleQuery);
    }
}
