using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class FloorRequirementsTests
{
    [Fact]
    public void FarmingFloorsAreIndependentAndRaiseTheScope()
    {
        var query = new QuerySettings { MaximumDepth = 4 };
        foreach (var depth in new[] { 22, 7, 17 }) query.ToggleFarmingFloor(depth);
        Assert.Equal(22, query.MaximumDepth);
        Assert.Equal(new[] { 7, 17, 22 }, query.FloorRequirements.Select(floor => floor.Depth));
        Assert.All(query.FloorRequirements, floor => Assert.True(floor.IsFarming));
        Assert.True(query.HasRequirements);
        Assert.Equal(3, query.SlotCount);
        Assert.Null(QueryRelationships.Validate(query));
        query.ToggleFarmingFloor(17);
        Assert.Equal(new[] { 7, 22 }, query.FloorRequirements.Select(floor => floor.Depth));
        Assert.Equal(22, query.MaximumDepth);
        query.MaximumDepth = 16;
        Assert.NotNull(QueryRelationships.Validate(query));
    }

    [Fact]
    public void FloorQueriesSurviveAllFormatsAndCopies()
    {
        var query = new QuerySettings();
        foreach (var depth in FloorRequirement.FarmingFloors) query.ToggleFarmingFloor(depth);
        query.FloorRequirements.Add(new() { Depth = 9, Feeling = "secrets", Rooms = ["secret_library"], AnyRooms = ["garden", "secret_garden"] });
        var document = ResultsExport.EncodeQueryDocument(query);
        foreach (var restored in new[] {
            query.Clone(), JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!,
            ResultsExport.DecodeQueryDocument(document),
            ResultsExport.DecodeQueryDocument(NativeEngine.TryDecodeShareText(NativeEngine.TryEncodeShareLink(document)!)!),
            ResultsExport.Decode(ResultsExport.Encode(query, ["AAA-AAA-AAA"], "test")).Query,
        }) Assert.Equal(document, ResultsExport.EncodeQueryDocument(restored));
        var copy = query.Clone(); copy.FloorRequirements[0].AnyRooms[0] = "library";
        Assert.Equal("garden", query.FloorRequirements[0].AnyRooms[0]);
        Assert.Empty(JsonSerializer.Deserialize<QuerySettings>("""{"Requirements":[]}""")!.FloorRequirements);
    }

    [Fact]
    public void NativeFilteringAndScoutEnforceFloorOnlyConditions()
    {
        var engine = new NativeEngine();
        var world = engine.Scout("AAA-AAA-AAA", 0);
        var feeling = world.FloorFeelings!.Single(floor => floor.Depth == 7).Feeling.ToString().ToLowerInvariant();
        var query = new QuerySettings { FloorRequirements = [new() { Depth = 7, Feeling = feeling }] };
        Assert.Equal(new[] { world.Seed }, engine.FilterSeeds(query, [world.Seed]));
        Assert.Equal(1, NativeEngine.ScoutMatches(world.Seed, 0, query).MatchedRequirements);
        query.FloorRequirements = [new() { Depth = 7, Feeling = feeling == "dark" ? "water" : "dark" }];
        Assert.Empty(engine.FilterSeeds(query, [world.Seed]));
        Assert.Equal(0, NativeEngine.ScoutMatches(world.Seed, 0, query).MatchedRequirements);
    }
}
