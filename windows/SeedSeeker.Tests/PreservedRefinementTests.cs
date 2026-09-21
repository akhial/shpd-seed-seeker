using Xunit;

namespace SeedSeeker.Tests;

public sealed class PreservedRefinementTests
{
    [Fact]
    public void FilteringReappliesTheOriginalTrinket()
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
    }
}
