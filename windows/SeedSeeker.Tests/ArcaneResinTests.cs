using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class ArcaneResinTests
{
    [Fact]
    public void ResinOnlyQueriesKeepTheirAmountAndFiltersAcrossFormats()
    {
        foreach (var amount in new[] { 1, 3, 65535 })
        foreach (var filter in new[] { new ArcaneResinFilter(), new ArcaneResinFilter(false, 12, ScoutItemSource.WandmakerReward) })
        {
            var query = new QuerySettings { ArcaneResin = amount, ArcaneResinFilter = filter };
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
        Assert.True(QueryRefinement.CanRefine(harder, query));
        Assert.False(QueryRefinement.CanRefine(query, harder));
    }
}
