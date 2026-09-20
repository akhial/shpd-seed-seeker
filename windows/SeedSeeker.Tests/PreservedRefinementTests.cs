using Xunit;

namespace SeedSeeker.Tests;

public sealed class PreservedRefinementTests
{
    [Fact]
    public async Task ARefinedScanReappliesTheOriginalTrinket()
    {
        var baseline = ResultsExport.DecodeQueryDocument("""
            {"auto_apply_trinket":true,"max_depth":19,"requirements":[
              {"item":"runic_blade","upgrade":1,"effect":"Grim"}]}
            """);
        var narrowed = ResultsExport.DecodeQueryDocument("""
            {"auto_apply_trinket":true,"max_depth":19,"requirements":[
              {"item":"runic_blade","upgrade":1,"effect":"Grim"},
              {"item":"whip","effect":"Venomous"}]}
            """);
        const string seed = "EYY-RUL-LQG";
        var engine = new NativeEngine();
        Assert.Null(Assert.Single(engine.FilterRecipes(baseline, baseline, [new(seed, 1)])).SelectedTrinket);
        Assert.Equal("parchment_scrap", Assert.Single(engine.FilterRecipes(narrowed, baseline, [new(seed, 1)])).SelectedTrinket);
        using var session = engine.StartRefined(narrowed, baseline, (checked((long)SeedCode.Value(seed)), 1), 1);
        var found = new List<SeedResult>();
        var deadline = DateTime.UtcNow.AddSeconds(10);
        while (true)
        {
            found.AddRange(session.PollRecipes(16));
            if (session.Status().State != SearchState.Running) break;
            Assert.True(DateTime.UtcNow < deadline, "Refined single-seed search timed out");
            await Task.Delay(10);
        }
        var result = Assert.Single(found);
        Assert.Equal(seed, result.Seed);
        Assert.Equal("parchment_scrap", result.SelectedTrinket);
        Assert.Equal(0, session.ResumeHint().Remaining);
    }
}
