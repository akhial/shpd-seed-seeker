using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The queries the app ships as read-only presets: every one must pass the same
/// checks Start runs before the engine is asked — the query's own and the
/// board's — and the two vault presets must ask for the levels only the Imp's
/// vault reaches.
/// </summary>
public sealed class BuiltInPresetsTests
{
    [Fact]
    public void EveryPresetIsARunnableQuery()
    {
        foreach (var preset in BuiltInPresets.All)
        {
            // Applied as the window applies a preset: to a copy, taken in by the board.
            var query = preset.Query.Clone();
            var editor = new BoardEditor();
            editor.Load(query);
            Assert.Null(editor.Problem(query));
        }
    }

    [Fact]
    public void Staff22AsksForTheVaultWand()
    {
        var preset = Assert.Single(BuiltInPresets.All, entry => entry.Name == "+22 Staff");
        var requirements = preset.Query.Requirements;
        Assert.Equal(BuiltInPresets.VaultFloorLimit, preset.Query.MaximumDepth);
        Assert.All(requirements, requirement => Assert.Equal(ItemKind.Wand, requirement.Kind));
        Assert.Equal([UpgradeMatch.Exactly, UpgradeMatch.Any, UpgradeMatch.Any, UpgradeMatch.AtLeast],
            requirements.Select(requirement => requirement.UpgradeMatch));
        Assert.Equal([4, 0, 0, 1], requirements.Select(requirement => requirement.Upgrade));
        Assert.Equal(new int?[] { 1, 1, 1, null }, requirements.Select(requirement => requirement.IdentityGroup));
    }

    [Fact]
    public void Tier4WeaponStacksTwoCopiesOnAPlusFive()
    {
        var preset = Assert.Single(BuiltInPresets.All, entry => entry.Name == "+26 Tier 4 Weapon");
        var requirements = preset.Query.Requirements;
        Assert.Equal(BuiltInPresets.VaultFloorLimit, preset.Query.MaximumDepth);
        Assert.Equal(3, requirements.Count);
        Assert.All(requirements, requirement =>
        {
            Assert.Equal(ItemKind.Weapon, requirement.Kind);
            Assert.Equal(1, requirement.IdentityGroup);
        });
        Assert.Equal(TierMatch.Exactly, requirements[0].TierMatch);
        Assert.Equal(4, requirements[0].Tier);
        Assert.Equal(UpgradeMatch.Exactly, requirements[0].UpgradeMatch);
        Assert.Equal(5, requirements[0].Upgrade);
        // Only the anchor may constrain the item a stack binds to: the board
        // folds the plain copies into one ×3 chip.
        var query = preset.Query.Clone();
        var item = Assert.Single(new BoardEditor().View(query).Entries);
        var chip = Assert.Single(item.Chips);
        Assert.Equal(3, chip.Stack.Count);
        Assert.Equal("×3", chip.CountBadge!.Text);
        Assert.Null(item.Problem);
    }
}
