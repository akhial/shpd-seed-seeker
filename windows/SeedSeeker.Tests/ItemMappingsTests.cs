using System.Text;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class ItemMappingsTests
{
    [Fact]
    public void NativeScoutReturnsEveryMappingInGameClassOrder()
    {
        var artwork = ItemMappingArtwork.Shared;
        Assert.Equal(new[] { "potions", "rings", "scrolls" }, artwork.Categories.Keys.Order());
        Assert.All(artwork.Categories.Values, art => Assert.Equal(12, art.IconSizes.Length));
        var engine = new NativeEngine();
        var world = engine.Scout("ABC-DEF-GHI", 0);
        var mappings = Assert.IsType<ScoutItemMappings>(world.ItemMappings);
        Assert.Equal(new ScoutItemMapping("Scroll of upgrade", "TIWAZ", 315), mappings.Scrolls[0]);
        foreach (var group in new[] { mappings.Scrolls, mappings.Potions, mappings.Rings })
        {
            Assert.Equal(12, group.Count);
            Assert.Equal(12, group.Select(x => x.SpriteIndex).Distinct().Count());
            Assert.Equal(12, group.Select(x => x.Appearance).Distinct().Count());
        }
        Assert.Equal(world.Gems.Ordinals.Select(x => (int)x), mappings.Rings.Select(x => x.SpriteIndex - 224));
        var challenged = engine.Scout(world.Seed, 1, trinket: "none").ItemMappings!;
        Assert.Equal(mappings.Scrolls, challenged.Scrolls);
        Assert.Equal(mappings.Potions, challenged.Potions);
        Assert.Equal(mappings.Rings, challenged.Rings);
    }

    private static byte[] Packet(string version = "SSC7", string mutation = "")
    {
        var w = new Writer();
        w.Bytes(Encoding.UTF8.GetBytes(version)); w.U8(11); w.Bytes(Encoding.UTF8.GetBytes("AAA-AAA-AAA"));
        w.Bytes(Enumerable.Range(0, 12).Select(x => (byte)x)); w.U8(0); w.U16(0);
        if (version == "SSC3") return w.Finish();
        w.U8(17); foreach (var item in ItemCatalog.For(ItemKind.Trinket)) w.Text(item.Id);
        if (version == "SSC4") return w.Finish();
        w.U8(0);
        if (version == "SSC5") return w.Finish();
        w.Text("");
        if (version == "SSC6") return w.Finish();
        foreach (var spriteBase in new[] { 304, 352, 224 })
        for (var index = 0; index < 12; index++)
        {
            w.Text(mutation == "blank" ? "" : $"Item {index}");
            w.Text(mutation == "duplicate" ? "Same" : $"Appearance {index}");
            w.U16(mutation == "range" ? 65535 : spriteBase + (mutation == "ring" && spriteBase == 224 ? (index + 1) % 12 : index));
        }
        return w.Finish();
    }

    [Fact]
    public void LegacyPacketsOmitMappingsAndMalformedExtensionsFail()
    {
        foreach (var version in new[] { "SSC3", "SSC4", "SSC5", "SSC6" })
            Assert.Null(NativeEngine.DecodeScout(Packet(version)).ItemMappings);
        var packet = Packet();
        Assert.NotNull(NativeEngine.DecodeScout(packet).ItemMappings);
        Assert.Throws<InvalidDataException>(() => NativeEngine.DecodeScout(packet[..^1]));
        Assert.Throws<InvalidDataException>(() => NativeEngine.DecodeScout([.. packet, 0]));
        foreach (var mutation in new[] { "blank", "duplicate", "range", "ring" })
            Assert.Throws<InvalidDataException>(() => NativeEngine.DecodeScout(Packet(mutation: mutation)));
    }
}
