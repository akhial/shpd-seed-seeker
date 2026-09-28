using System.Collections.ObjectModel;
using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The model around the requirement board: the "any of these" slots the
/// document and the scout count, effect sets, and the persisted schema's
/// backward compatibility. The board itself — folding, edits, chip text and
/// problems — is the shared editor's, covered by the core's own tests and,
/// across the bridge, by <see cref="RequirementBoardTests"/>.
/// </summary>
public sealed class QueryRelationshipsTests
{
    private static ObservableCollection<ItemRequirement> List(params ItemRequirement[] requirements) => new(requirements);

    private static ItemRequirement Wand(int? group = null) => new() { Kind = ItemKind.Wand, AlternativeGroup = group };

    [Fact]
    public void SlotsCollapseAlternativesAtTheirFirstMembersPosition()
    {
        var a = Wand(1); var b = Wand(); var c = Wand(1); var d = Wand(2); var e = Wand(2);
        var slots = QueryRelationships.Slots([a, b, c, d, e]);
        Assert.Equal([[a, c], [b], [d, e]], slots.Select(slot => slot.ToArray()));
        Assert.Equal(3, QueryRelationships.SlotCount([a, b, c, d, e]));
        Assert.Equal(0, QueryRelationships.SlotCount([]));
    }

    [Fact]
    public void TheSingleEffectViewStaysInStepWithTheFilter()
    {
        var requirement = new ItemRequirement { Kind = ItemKind.Weapon, Modifier = "Blazing" };
        Assert.Equal(["Blazing"], requirement.Effect.Effects);
        Assert.Equal("Blazing", requirement.Modifier);
        requirement.Effect = EffectFilter.OneOf(["Blazing", "Chilling"]);
        Assert.Null(requirement.Modifier);
        // Setting null never erases a wider filter.
        requirement.Modifier = null;
        Assert.Equal(2, requirement.Effect.Effects.Count);
        requirement.Effect = EffectFilter.Enchantment();
        Assert.Null(requirement.Modifier);
        Assert.True(EffectFilter.OneOf(ItemCatalog.Enchantments).IsEveryEnchantmentOf(ItemKind.ThrownWeapon));
        Assert.False(EffectFilter.OneOf(ItemCatalog.Enchantments).IsEveryEnchantmentOf(ItemKind.Armor));
        Assert.Equal(["Blazing"], EffectFilter.OneOf(["Blazing", "Annoying"]).WithoutCurses(ItemKind.Weapon).Effects);
    }

    [Fact]
    public void CloningCopiesTheEffectFilterRatherThanSharingIt()
    {
        var original = new ItemRequirement { Kind = ItemKind.Weapon, Effect = EffectFilter.OneOf(["Blazing"]), LevelSum = new(1, 2), AlternativeGroup = null };
        var copy = original.Clone();
        copy.Effect.Effects.Add("Chilling");
        Assert.Equal(["Blazing"], original.Effect.Effects);
        Assert.Equal(original.LevelSum, copy.LevelSum);
    }

    [Fact]
    public void SavedQueriesFromBeforeEffectSetsStillLoad()
    {
        // The shape MainWindow persisted before this change: a bare Modifier,
        // no Effect, AlternativeGroup or LevelSum. "FastMode" is deliberately
        // still here — settings and presets saved before that mode was retired
        // must keep loading, the flag ignored.
        const string legacy = """
            { "Requirements": [ { "Key": 7, "Item": null, "Upgrade": 2, "Modifier": "Blazing", "Kind": 0, "Tier": 0,
              "TierMatch": 0, "UpgradeMatch": 1, "Source": null, "IdentityGroup": 1, "MaximumDepth": 9, "RequireUncursed": true } ],
              "MaximumDepth": 12, "RequireBlacksmith": false, "ExcludeBlacksmithRewards": false, "WandmakerQuest": 0, "FastMode": true, "Challenges": 0 }
            """;
        var query = JsonSerializer.Deserialize<QuerySettings>(legacy)!;
        Assert.Equal(12, query.MaximumDepth);
        var requirement = Assert.Single(query.Requirements);
        Assert.Equal("Blazing", requirement.Modifier);
        Assert.Equal(["Blazing"], requirement.Effect.Effects);
        Assert.Null(requirement.AlternativeGroup); Assert.Null(requirement.LevelSum);
        Assert.Equal(1, requirement.IdentityGroup); Assert.True(requirement.RequireUncursed);
    }

    [Fact]
    public void TheNewFieldsPersistThroughTheSavedSchema()
    {
        var query = new QuerySettings
        {
            Requirements = List(
                new() { Kind = ItemKind.Weapon, Effect = EffectFilter.OneOf(["Blocking", "Projecting"]), AlternativeGroup = 1 },
                new() { Kind = ItemKind.Armor, Effect = EffectFilter.Enchantment(), AlternativeGroup = 1 },
                new() { Kind = ItemKind.Ring, LevelSum = new(2, 4) }),
        };
        var again = JsonSerializer.Deserialize<QuerySettings>(JsonSerializer.Serialize(query))!;
        Assert.Equal(["Blocking", "Projecting"], again.Requirements[0].Effect.Effects);
        Assert.Equal(1, again.Requirements[0].AlternativeGroup);
        Assert.True(again.Requirements[1].Effect.AnyEnchantment);
        Assert.Equal(new LevelSum(2, 4), again.Requirements[2].LevelSum);
        Assert.Equal(ResultsExport.EncodeQueryDocument(query), ResultsExport.EncodeQueryDocument(again));
    }
}
