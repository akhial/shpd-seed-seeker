// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;

namespace SeedSeeker;

public sealed record ScoutItemMapping(string Name, string Appearance, int SpriteIndex)
{
    public string Label => $"{Appearance} — {Name}";
}

public sealed record ScoutItemMappings(IReadOnlyList<ScoutItemMapping> Scrolls,
    IReadOnlyList<ScoutItemMapping> Potions, IReadOnlyList<ScoutItemMapping> Rings);

internal sealed record MappingArt(int[] SpriteSize, int IconBase, int[][] IconSizes);
internal sealed record ItemMappingArtwork(int SlotSize, int SlotGap, Dictionary<string, MappingArt> Categories)
{
    public static ItemMappingArtwork Shared { get; } = JsonSerializer.Deserialize<ItemMappingArtwork>(
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Assets", "item-mapping-art.json")),
        new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
        ?? throw new InvalidDataException("Missing journal artwork metadata");
}
