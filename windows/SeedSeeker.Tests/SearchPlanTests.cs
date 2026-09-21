using Xunit;
namespace SeedSeeker.Tests;

public sealed class SearchPlanTests
{
    [Fact]
    public void EveryDiscoveryAndImportJoinsThePoolWithoutReplacingEarlierRecipes()
    {
        var a = ResultsExport.DecodeQueryDocument("""{"requirements":[{"kind":"wand"}]}""");
        var b = ResultsExport.DecodeQueryDocument("""{"requirements":[{"kind":"armor"}]}""");
        var first = new SeedResult("AAA-AAA-AAA", 1);
        var pool = TargetRun.Remember(null, a, [first, new("BBB-BBB-BBB", 1)]);
        pool = TargetRun.Remember(pool, b, [first with { SelectedTrinket = "mossy_clump" }, new("CCC-CCC-CCC", 1)]);
        Assert.Equal(["AAA-AAA-AAA", "BBB-BBB-BBB", "CCC-CCC-CCC"], pool.Seeds);
        Assert.Equal(first, pool.Recipes![first.Seed]);
        Assert.Equal(ResultsExport.EncodeQueryDocument(a), ResultsExport.EncodeQueryDocument(pool.Sources![first.Seed]));
        Assert.Equal(ResultsExport.EncodeQueryDocument(b), ResultsExport.EncodeQueryDocument(pool.Sources!["CCC-CCC-CCC"]));
        Assert.Equal(pool.Seeds, TargetRun.Remember(pool, a, []).Seeds);
    }
}
