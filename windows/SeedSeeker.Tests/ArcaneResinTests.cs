using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class ArcaneResinTests
{
    [Fact]
    public void ResinOnlyQueriesKeepTheirAmountAndFiltersAcrossFormats()
    {
        foreach (var amount in new[] { 0, 1, 3, 65535 })
        foreach (var filter in new[] { new ArcaneResinFilter(), new ArcaneResinFilter(false, 12, ScoutItemSource.WandmakerReward), new ArcaneResinFilter(IncludeMageWand: true) })
        {
            var query = new QuerySettings { ArcaneResin = amount, ArcaneResinAuto = amount == 0, ArcaneResinFilter = filter };
            Assert.True(query.HasRequirements);
            Assert.Equal(1, query.SlotCount);
            Assert.Null(QueryRelationships.Validate(query));
            var document = ResultsExport.EncodeQueryDocument(query);
            foreach (var restored in new[] {
                query.Clone(), JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!,
                ResultsExport.DecodeQueryDocument(document), ResultsExport.DecodeQueryDocument(NativeEngine.TryDecodeShareText(NativeEngine.TryEncodeShareLink(document)!)!),
                ResultsExport.Decode(ResultsExport.Encode(query, ["AAA-AAA-AAA"], "test")).Query,
            }) Assert.Equal(document, ResultsExport.EncodeQueryDocument(restored));
        }
        Assert.Equal(new ArcaneResinFilter(), ResultsExport.DecodeQueryDocument("""{"requirements":[],"arcane_resin":6}""").ArcaneResinFilter);
        Assert.Equal(0, JsonSerializer.Deserialize<QuerySettings>("""{"Requirements":[]}""")!.ArcaneResin);
        Assert.NotNull(QueryRelationships.Validate(new() { ArcaneResin = -1 }));
        Assert.NotNull(QueryRelationships.Validate(new() { ArcaneResin = 65536 }));
        Assert.NotNull(QueryRelationships.Validate(new() { ArcaneResin = 2, ArcaneResinFilter = new(MaximumDepth: 25) }));
    }

    [Fact]
    public void ExcludedReforgeStackAndMageCreditSurviveAllFormats()
    {
        var query = ResultsExport.DecodeQueryDocument("""{"arcane_resin":"auto","arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"item":"wand_frost","exclude_resin":true},{"item":"wand_frost"},{"item":"wand_frost"}],"floor_requirements":[{"depth":7,"feeling":"dark"}]}""");
        Assert.True(query.ArcaneResinFilter.IncludeMageWand);
        Assert.Equal(new[] { true, false, false }, query.Requirements.Select(r => r.ExcludeResin));
        Assert.Equal(3, Assert.Single(QueryRelationships.BoardItems(query.Requirements)).StackCount);
        Assert.Null(QueryRelationships.Validate(query));
        var document = ResultsExport.EncodeQueryDocument(query);
        foreach (var restored in new[] {
            query.Clone(), JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!,
            ResultsExport.DecodeQueryDocument(NativeEngine.TryDecodeShareText(NativeEngine.TryEncodeShareLink(document)!)!),
            ResultsExport.Decode(ResultsExport.Encode(query, [], "test")).Query,
        }) Assert.Equal(document, ResultsExport.EncodeQueryDocument(restored));
        var baseline = query.Clone(); baseline.ArcaneResinAuto = false;
        using var search = new NativeEngine().StartResumed(query, 0, 0, 1);
        using var plain = new NativeEngine().StartResumed(baseline, 0, 0, 1);
        Assert.True(search.Status().Probability > 0);
        Assert.Equal(plain.Status().Probability, search.Status().Probability, 12);
        Assert.False(JsonSerializer.Deserialize<ArcaneResinFilter>("""{"Uncursed":true}""")!.IncludeMageWand);
        query.Requirements = new([new ItemRequirement { Kind = ItemKind.Ring, ExcludeResin = true }]);
        Assert.NotNull(QueryRelationships.Validate(query));
        query.Requirements[0].Kind = ItemKind.Wand; query.Requirements[0].Blanket = true;
        Assert.NotNull(QueryRelationships.Validate(query));
    }

    [Fact]
    public void NativeScoutAndRefinementUseResinEvenWithoutItemRequirements()
    {
        var query = new QuerySettings { ArcaneResin = 2 };
        var world = new NativeEngine().Scout("AAA-AAA-AAA", 0);
        var marks = NativeEngine.ScoutMatches(world.Seed, 0, query);
        Assert.Equal(1, marks.MatchedRequirements);
        Assert.Equal(1, marks.TotalRequirements);
        Assert.NotEmpty(marks.Matched);
        Assert.All(marks.Matched, index => Assert.Equal(ItemKind.Wand, world.Items[index].Item.Kind));
        var harder = query.Clone(); harder.ArcaneResin = 65535;
        Assert.Equal(0, NativeEngine.ScoutMatches(world.Seed, 0, harder).MatchedRequirements);
    }
    [Fact]
    public void AutoBlanketSharesItsWitnessAndPreservesTheEngineEstimate()
    {
        var query = ResultsExport.DecodeQueryDocument("""{"arcane_resin":"auto","requirements":[{"item":"wand_lightning","upgrade":2},{"kind":"wand","upgrade":2,"blanket":true}]}""");
        Assert.True(query.ArcaneResinAuto);
        var document = ResultsExport.EncodeQueryDocument(query);
        Assert.Equal(document, ResultsExport.EncodeQueryDocument(ResultsExport.DecodeQueryDocument(
            NativeEngine.TryDecodeShareText(NativeEngine.TryEncodeShareLink(document)!)!)));
        Assert.Equal(3, query.SlotCount);
        var marks = NativeEngine.ScoutMatches("AAA-AAA-AAS", 0, query);
        Assert.Equal(3, marks.MatchedRequirements);
        Assert.Equal(3, marks.TotalRequirements);
        var direct = query.Clone(); direct.Requirements = new(direct.Requirements.Where(r => !r.Blanket));
        using var search = new NativeEngine().StartResumed(query, 18, 0, 1);
        using var directSearch = new NativeEngine().StartResumed(direct, 18, 0, 1);
        Assert.True(search.Status().Probability > 0);
        Assert.Equal(directSearch.Status().Probability, search.Status().Probability, 12);
        query.Requirements = new(query.Requirements.Where(r => r.Blanket));
        Assert.NotNull(QueryRelationships.Validate(query));
    }

}
