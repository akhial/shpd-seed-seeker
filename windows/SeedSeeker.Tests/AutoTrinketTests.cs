using Xunit;

namespace SeedSeeker.Tests;

public sealed class AutoTrinketTests
{
    private static QuerySettings Query() => ResultsExport.DecodeQueryDocument("""
        {"max_depth":19,"auto_apply_trinket":true,"requirements":[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]}
        """);

    [Fact]
    public void DefaultsAndRecipesSurviveDocumentsAndClones()
    {
        var query = Query();
        Assert.True(query.Clone().AutoApplyTrinket);
        Assert.All(BuiltInPresets.All, preset => Assert.True(preset.Query.AutoApplyTrinket));
        Assert.False(ResultsExport.DecodeQueryDocument("""{"requirements":[{"item":"runic_blade"}]}""").AutoApplyTrinket);
        var restored = ResultsExport.Decode(ResultsExport.Encode(query, ["SRU-YSU-QHS", "EYY-RUL-LQG"], "test", ["parchment_scrap", null]));
        Assert.True(restored.Query.AutoApplyTrinket);
        Assert.Equal(new string?[] { "parchment_scrap", null }, restored.Trinkets);
    }

    [Fact]
    public void NativeFilterStripsOnlyUnnecessaryRecipesAndRefineRestoresNeededChoice()
    {
        var engine = new NativeEngine(); var query = Query();
        var matches = engine.FilterRecipes(query, query, [new("SRU-YSU-QHS", 1, "parchment_scrap"), new("EYY-RUL-LQG", 2, "parchment_scrap")]);
        Assert.Equal(new string?[] { "parchment_scrap", null }, matches.Select(recipe => recipe.SelectedTrinket));
        foreach (var match in matches)
        {
            var trinket = match.SelectedTrinket ?? "none";
            var marked = NativeEngine.ScoutMatches(match.Seed, 0, query, trinket);
            Assert.Equal(marked.TotalRequirements, marked.MatchedRequirements);
        }
        var refined = query.Clone(); refined.Requirements.Add(new() { Item = ItemCatalog.Find("whip"), Effect = EffectFilter.OneOf(["Venomous"]), Kind = ItemKind.Weapon, UpgradeMatch = UpgradeMatch.Any });
        var restored = Assert.Single(engine.FilterRecipes(refined, query, [matches[1]]));
        Assert.Equal("parchment_scrap", restored.SelectedTrinket);
        Assert.Equal(0, NativeEngine.ScoutMatches(matches[0].Seed, 0, query, "none").MatchedRequirements);
    }
}
