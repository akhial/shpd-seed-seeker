using System.Text.Json;
using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The catalog asset the app ships, which is the Android app's file linked
/// rather than copied. The item entries and the four effect tables are read
/// from it, so a catalog bump carries both and neither can fall behind.
/// </summary>
public sealed class ItemCatalogTests
{
    /// <summary>The asset in the repository, read independently of ItemCatalog.</summary>
    private static JsonElement Asset()
    {
        var root = NativeEngineLibrary.WorkspaceRoot()
            ?? throw new InvalidOperationException("Could not locate the workspace root.");
        var path = Path.Combine(root, "android", "app", "src", "main", "assets", "third_party",
            "shattered-pixel-dungeon", "catalog-v4.0.0.json");
        return JsonDocument.Parse(File.ReadAllText(path)).RootElement;
    }

    private static string[] Table(string name) =>
        [.. Asset().GetProperty("modifiers").GetProperty(name).EnumerateArray().Select(value => value.GetString()!)];

    [Fact]
    public void TheEffectTablesAreTheAssetsOwn()
    {
        Assert.Equal(Table("weaponEnchantments"), ItemCatalog.Enchantments);
        Assert.Equal(Table("weaponCurses"), ItemCatalog.WeaponCurses);
        Assert.Equal(Table("armorGlyphs"), ItemCatalog.Glyphs);
        Assert.Equal(Table("armorCurses"), ItemCatalog.ArmorCurses);
        // Non-empty, so an asset the deserializer failed to read cannot pass.
        Assert.NotEmpty(ItemCatalog.Enchantments);
        Assert.NotEmpty(ItemCatalog.WeaponCurses);
        Assert.NotEmpty(ItemCatalog.Glyphs);
        Assert.NotEmpty(ItemCatalog.ArmorCurses);
    }

    [Fact]
    public void ModifiersOffersEnchantmentsThenCursesPerFamily()
    {
        Assert.Equal([.. ItemCatalog.Enchantments, .. ItemCatalog.WeaponCurses], ItemCatalog.Modifiers(ItemKind.Weapon));
        Assert.Equal(ItemCatalog.Modifiers(ItemKind.Weapon), ItemCatalog.Modifiers(ItemKind.ThrownWeapon));
        Assert.Equal([.. ItemCatalog.Glyphs, .. ItemCatalog.ArmorCurses], ItemCatalog.Modifiers(ItemKind.Armor));
        Assert.Empty(ItemCatalog.Modifiers(ItemKind.Wand));
        Assert.Empty(ItemCatalog.Modifiers(ItemKind.Ring));
    }

    [Fact]
    public void IsCurseAsksTheCurseTableOfTheItemsFamily()
    {
        Assert.All(ItemCatalog.WeaponCurses, curse => Assert.True(ItemCatalog.IsCurse(ItemKind.MeleeWeapon, curse)));
        Assert.All(ItemCatalog.Enchantments, effect => Assert.False(ItemCatalog.IsCurse(ItemKind.Weapon, effect)));
        Assert.All(ItemCatalog.ArmorCurses, curse => Assert.True(ItemCatalog.IsCurse(ItemKind.Armor, curse)));
        Assert.All(ItemCatalog.Glyphs, glyph => Assert.False(ItemCatalog.IsCurse(ItemKind.Armor, glyph)));
    }

    [Fact]
    public void TheItemsAreTheAssetsOwn()
    {
        var entries = Asset().GetProperty("entries").EnumerateArray()
            .Select(entry => entry.GetProperty("id").GetString()).ToArray();
        Assert.Equal(entries, ItemCatalog.All.Select(item => item.Id));
    }

    /// <summary>
    /// The ring glyphs are the asset's <c>typeIcon</c>, carried through rather
    /// than derived from the sprite index: once a scouted ring is drawn in the
    /// cell its run's gem picks, that derivation names the wrong ring.
    /// </summary>
    [Fact]
    public void TheRingGlyphsAreTheAssetsOwn()
    {
        var glyphs = Asset().GetProperty("entries").EnumerateArray()
            .Select(entry => entry.TryGetProperty("typeIcon", out var icon) ? icon.GetInt32() : (int?)null)
            .ToArray();
        Assert.Equal(glyphs, ItemCatalog.All.Select(item => item.TypeIconIndex));
        // Every ring carries one and nothing else does, and each is its class's
        // own offset into the catalog's block of ring cells.
        foreach (var item in ItemCatalog.All)
        {
            Assert.Equal(item.Kind == ItemKind.Ring, item.TypeIconIndex is not null);
            if (item.TypeIconIndex is int glyph) Assert.Equal(RingGems.RingSpriteBase + glyph, item.SpriteIndex);
        }
    }
}
