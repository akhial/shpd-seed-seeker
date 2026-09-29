using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The queries the app ships as read-only presets: every one must pass the same
/// checks Start runs before the engine is asked — the query's own and the
/// board's — and each must be exactly the query its shared document decodes to,
/// so the literals cannot drift from the presets the other clients ship.
/// </summary>
public sealed class BuiltInPresetsTests
{
    /// <summary>The presets as shared query documents, in picker order.</summary>
    private static readonly (string Id, string Name, string Document)[] Documents =
    [
        ("disintegrate", "DISINTEGRATE", """
            {"max_depth":19,"auto_apply_trinket":true,"requirements":[
                {"item":"wand_disintegration","kind":"wand","upgrade":{"at_least":3}},
                {"item":"wand_disintegration","kind":"wand"},
                {"item":"wand_disintegration","kind":"wand"},
                {"item":"eye_of_newt","kind":"trinket","trinket_transmutations":1},
                {"item":"ring_energy","kind":"ring","upgrade":{"at_least":2}}]}
            """),
        ("guerilla-assassin", "Guerilla Assassin", """
            {"auto_apply_trinket":true,"requirements":[
                {"item":"assassins_blade","kind":"weapon","effect":"Blooming","max_depth":7,"upgrade":3},
                {"kind":"armor","effect":"Camouflage"},
                {"item":"ring_arcana","kind":"ring","upgrade":{"at_least":2}}]}
            """),
        ("ring-of-wealth", "Ring of Wealth", """
            {"auto_apply_trinket":true,
             "floor_requirements":[{"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}],
             "requirements":[
                {"item":"ring_wealth","kind":"ring","upgrade":4},
                {"item":"dried_rose","kind":"artifact","max_depth":9},
                {"kind":"armor","max_depth":4,"tier":{"at_most":4},"upgrade":3},
                {"kind":"weapon","max_depth":9,"tier":{"at_most":4},"upgrade":3},
                {"item":"dimensional_sundial","kind":"trinket","trinket_transmutations":1}]}
            """),
        ("necromancer", "Necromancer", """
            {"max_depth":14,"wandmaker_quest":"corpse_dust","auto_apply_trinket":true,"requirements":[
                {"item":"wand_corruption","kind":"wand","upgrade":3},
                {"kind":"weapon","tier":{"exact":5},"upgrade":3},
                {"item":"plate_armor","kind":"armor","upgrade":3}]}
            """),
        ("blood-berserker", "Blood Berserker", """
            {"auto_apply_trinket":true,"requirements":[
                {"kind":"weapon","tier":{"exact":5},"upgrade":3,"effect":"Vampiric"},
                {"effect":"Thorns","item":"plate_armor","kind":"armor","upgrade":3},
                {"item":"ring_arcana","kind":"ring","upgrade":4},
                {"item":"chalice_of_blood","kind":"artifact"}]}
            """),
    ];

    private static QueryPreset Preset(string id) => Assert.Single(BuiltInPresets.All, entry => entry.Id == id);

    [Fact]
    public void ThePickerOffersTheFivePresetsInOrder()
    {
        Assert.Equal(Documents.Select(entry => (entry.Id, entry.Name)),
            BuiltInPresets.All.Select(preset => (preset.Id, preset.Name)));
        Assert.All(BuiltInPresets.All, preset => Assert.True(preset.IsBuiltIn));
    }

    [Fact]
    public void EveryPresetIsTheQueryItsDocumentDecodesTo()
    {
        foreach (var (id, _, document) in Documents)
            Assert.Equal(ResultsExport.EncodeQueryDocument(ResultsExport.DecodeQueryDocument(document)),
                ResultsExport.EncodeQueryDocument(Preset(id).Query));
    }

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
    public void DisintegrateAsksForThreeDisintegrationWandsOnePlusThree()
    {
        var query = Preset("disintegrate").Query;
        Assert.Equal(19, query.MaximumDepth);
        var wands = query.Requirements.Take(3).ToList();
        Assert.All(wands, wand => Assert.Equal("wand_disintegration", wand.Item?.Id));
        Assert.Equal([UpgradeMatch.AtLeast, UpgradeMatch.Any, UpgradeMatch.Any], wands.Select(wand => wand.UpgradeMatch));
        Assert.Equal(3, wands[0].Upgrade);
        var trinket = query.Requirements[3];
        Assert.Equal(ItemKind.Trinket, trinket.Kind);
        Assert.Equal("eye_of_newt", trinket.Item?.Id);
        Assert.Equal(1, trinket.TrinketTransmutations);
    }

    [Fact]
    public void GuerillaAssassinWantsAnEarlyBloomingBlade()
    {
        var requirements = Preset("guerilla-assassin").Query.Requirements;
        Assert.Equal("assassins_blade", requirements[0].Item?.Id);
        Assert.Equal("Blooming", requirements[0].Effect.Single);
        Assert.Equal(7, requirements[0].MaximumDepth);
        Assert.Equal(UpgradeMatch.Exactly, requirements[0].UpgradeMatch);
        Assert.Equal(3, requirements[0].Upgrade);
        Assert.Equal(ItemKind.Armor, requirements[1].Kind);
        Assert.Null(requirements[1].Item);
        Assert.Equal("Camouflage", requirements[1].Effect.Single);
    }

    [Fact]
    public void RingOfWealthFarmsFloor17AsTheToggleMarksIt()
    {
        var preset = Preset("ring-of-wealth");
        var floor = Assert.Single(preset.Query.FloorRequirements);
        Assert.Equal(17, floor.Depth);
        Assert.True(floor.IsFarming);
        var toggled = preset.Query.Clone();
        toggled.FloorRequirements.Clear();
        toggled.ToggleFarmingFloor(17);
        Assert.Equal(ResultsExport.EncodeQueryDocument(toggled), ResultsExport.EncodeQueryDocument(preset.Query));

        var requirements = preset.Query.Requirements;
        Assert.Equal(ItemKind.Artifact, requirements[1].Kind);
        Assert.Equal(9, requirements[1].MaximumDepth);
        Assert.All(requirements.Skip(2).Take(2), requirement =>
        {
            Assert.Null(requirement.Item);
            Assert.Equal(TierMatch.AtMost, requirement.TierMatch);
            Assert.Equal(4, requirement.Tier);
        });
        Assert.Equal(new int?[] { 4, 9 }, requirements.Skip(2).Take(2).Select(requirement => requirement.MaximumDepth));
    }

    [Fact]
    public void NecromancerDemandsTheCorpseDustQuest()
    {
        var query = Preset("necromancer").Query;
        Assert.Equal(14, query.MaximumDepth);
        Assert.Equal(WandmakerQuest.CorpseDust, query.WandmakerQuest);
        Assert.Equal("wand_corruption", query.Requirements[0].Item?.Id);
        Assert.Equal(TierMatch.Exactly, query.Requirements[1].TierMatch);
        Assert.Equal(5, query.Requirements[1].Tier);
    }

    [Fact]
    public void BloodBerserkerWantsAVampiricTier5WeaponAndThornsPlate()
    {
        var requirements = Preset("blood-berserker").Query.Requirements;
        Assert.Null(requirements[0].Item);
        Assert.Equal(TierMatch.Exactly, requirements[0].TierMatch);
        Assert.Equal(5, requirements[0].Tier);
        Assert.Equal("Vampiric", requirements[0].Effect.Single);
        Assert.Equal("plate_armor", requirements[1].Item?.Id);
        Assert.Equal("Thorns", requirements[1].Effect.Single);
        Assert.Equal(ItemKind.Artifact, requirements[3].Kind);
        Assert.Equal("chalice_of_blood", requirements[3].Item?.Id);
    }
}
