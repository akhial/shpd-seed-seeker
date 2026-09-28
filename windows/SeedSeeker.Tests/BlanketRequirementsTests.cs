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
            Assert.Null(new BoardEditor().Problem(restored));
        }
    }

    /// <summary>What the window sends when a blanket chip is dragged, stacked or joined, through the real board.</summary>
    [Fact]
    public void BlanketsStaySeparateFromStacksAndOrdinaryAlternatives()
    {
        var editor = new BoardEditor();
        var query = new QuerySettings { Requirements = [Wand(), Wand(true), Wand(true)] };
        editor.Load(query);
        var (ordinary, first, second) = (query.Requirements[0].Key, query.Requirements[1].Key, query.Requirements[2].Key);
        var board = editor.View(query);
        Assert.Equal(3, board.Entries.Count);
        Assert.Equal(new BoardCounts(1, 2), board.Counts);
        Assert.False(board.EntryOf(first)!.Stack.CanGrow);
        Assert.False(editor.Edit(query, BoardEdit.SetCount(first, 3)).Changed);
        // An ordinary chip joins nothing in the blanket section.
        Assert.DoesNotContain(first, board.ChipOf(ordinary)!.Join);
        Assert.Equal(DropEffect.None, board.Drop(ordinary, DropKind.Chip, first).Effect);
        Assert.False(editor.Edit(query, BoardEdit.Join(ordinary, first)).Changed);
        query.Requirements = new(editor.Edit(query, BoardEdit.Join(first, second)).Rows!);
        Assert.Equal(new BoardCounts(1, 1), editor.View(query).Counts);
        Assert.All(query.Requirements, r => Assert.Null(r.IdentityGroup));
        Assert.Null(editor.Problem(query));
        Assert.NotNull(NativeEngine.TryEncodeShareLink(ResultsExport.EncodeQueryDocument(query)));
        query.Requirements = new(editor.Edit(query, BoardEdit.Detach(first)).Rows!);
        Assert.Equal(new BoardCounts(1, 2), editor.View(query).Counts);
    }

    [Fact]
    public void InvalidBlanketsAreRejectedBeforeSearching()
    {
        // The list as it stands, not as a load would put it: a lone label is not dissolved first.
        static string? Problem(params ItemRequirement[] requirements) =>
            new BoardEditor().Problem(new QuerySettings { Requirements = new(requirements) });
        Assert.Equal("Add at least one ordinary requirement.", Problem(Wand(true)));
        var ordinary = Wand(); ordinary.AlternativeGroup = 1;
        var blanket = Wand(true); blanket.AlternativeGroup = 1;
        Assert.NotNull(Problem(ordinary, blanket));
        blanket = Wand(true); blanket.IdentityGroup = 1;
        Assert.NotNull(Problem(Wand(), blanket));
        blanket = Wand(true); blanket.SelectTrinket = true;
        Assert.NotNull(Problem(Wand(), blanket));
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
