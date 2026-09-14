// SPDX-License-Identifier: GPL-3.0-or-later
using System.Text.Json;
using System.Text.Json.Nodes;

namespace SeedSeeker;

/// <summary>The v3 scene is authoritative; contents metadata never becomes extra drawing.</summary>
public sealed class LevelMapDocument
{
    public string Format { get; init; } = "";
    public int SchemaVersion { get; init; }
    public string Profile { get; init; } = "";
    public string AssetRevision { get; init; } = "";
    public string Seed { get; init; } = "";
    public int Depth { get; init; }
    public int Branch { get; init; }
    public string Kind { get; init; } = "";
    public string? SelectedTrinket { get; init; }
    public int Width { get; init; }
    public int Height { get; init; }
    public int[][] SecretRooms { get; init; } = [];
    public int[] SecretDoors { get; init; } = [];
    public int[] SecretTraps { get; init; } = [];
    public MapBranch[] Branches { get; init; } = [];
    public MapAsset[] Assets { get; init; } = [];
    public MapScene Scene { get; init; } = new();
    public bool HasSecrets => SecretRooms.Length + SecretDoors.Length + SecretTraps.Length > 0;

    public static LevelMapDocument Parse(byte[] json)
    {
        var map = JsonSerializer.Deserialize<LevelMapDocument>(json, new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
            ?? throw new InvalidDataException("The engine returned an empty map.");
        if (map.Format != "seed-seeker-level-map" || map.SchemaVersion != 3)
            throw new InvalidDataException("This map format requires a newer version of Seed Seeker.");
        if (map.Width <= 0 || map.Height <= 0 || map.Width > 256 || map.Height > 256 || map.Scene.TileSize != 16
            || map.Scene.Layers.Length == 0 || map.Scene.ConcealedLayers.Length == 0
            || map.Scene.Layers.Concat(map.Scene.ConcealedLayers).Any(layer => layer.Cells.Length != map.Width * map.Height
                || layer.Cells.Any(index => index is int i && (i < 0 || i >= map.Scene.Sprites.Length)))
            || map.Scene.Sprites.Any(sprite => sprite.Frames.Length == 0 || sprite.FrameDurationMs <= 0))
            throw new InvalidDataException("The engine returned an invalid map scene.");
        return map;
    }

    public static string Request(string seed, int depth, int branch, QuerySettings query, string? trinket)
    {
        var request = new JsonObject
        {
            ["seed"] = seed, ["depth"] = depth, ["branch"] = branch,
            ["challenges"] = new JsonArray([.. Challenges.All.Where(c => (query.Challenges & c.Mask) != 0).Select(c => JsonValue.Create(c.Name))]),
            ["trinket"] = trinket,
        };
        if (query.Requirements.Count > 0) request["query"] = JsonNode.Parse(ResultsExport.EncodeQueryDocument(query));
        return request.ToJsonString();
    }
}

public sealed record MapBranch(int Depth, int Branch, string Kind, int Entrance);
public sealed record MapAsset(string Id, int Width, int Height, string Sha256);
public sealed class MapScene
{
    public int TileSize { get; init; }
    public MapSprite[] Sprites { get; init; } = [];
    public MapLayer[] Layers { get; init; } = [];
    public MapLayer[] ConcealedLayers { get; init; } = [];
    public MapEmitter[] Emitters { get; init; } = [];
    public MapEmitter[] ConcealedEmitters { get; init; } = [];
}
public sealed record MapLayer(string Name, string? Blend, int?[] Cells);
public sealed record MapSprite(double FrameDurationMs, MapDraw[][] Frames);
public sealed record MapGlow(int[] Color, double PeriodMs);
public sealed class MapDraw
{
    public string Kind { get; init; } = "";
    public string Asset { get; init; } = "";
    public double[] Source { get; init; } = [];
    public double[] Destination { get; init; } = [];
    public int[] Rgba { get; init; } = [];
    public int Opacity { get; init; } = 255;
    public int[]? Tint { get; init; }
    public MapGlow? Glow { get; init; }
}
public sealed record MapCurve(double[][] Points, bool Sqrt)
{
    public double Value(double progress)
    {
        var p = Math.Clamp(progress, 0, 1) * 1000;
        var right = Array.FindIndex(Points, point => point[0] >= p);
        double value;
        if (right <= 0) value = Points[right == 0 ? 0 : ^1][1];
        else
        {
            var a = Points[right - 1]; var b = Points[right];
            value = a[1] + (b[1] - a[1]) * (p - a[0]) / (b[0] - a[0]);
        }
        return Sqrt ? Math.Sqrt(Math.Max(0, value / 1000)) : value / 1000;
    }
}
public sealed class MapEmitter
{
    public double? StartMs { get; init; }
    public bool WallMask { get; init; }
    public bool ClipToChasm { get; init; }
    public int Cell { get; init; }
    public double LoopMs { get; init; }
    public string? Blend { get; init; }
    public MapDraw Image { get; init; } = new();
    public double[] Velocity { get; init; } = [0, 0];
    public double[] Acceleration { get; init; } = [0, 0];
    public double AngularSpeed { get; init; }
    public MapCurve Alpha { get; init; } = new([[0, 1000]], false);
    public MapCurve Scale { get; init; } = new([[0, 1000]], false);
    public MapCurve? ScaleX { get; init; }
    public MapCurve? ScaleY { get; init; }
    public MapParticle[] Particles { get; init; } = [];

    public MapParticleState? State(MapParticle particle, double elapsed)
    {
        var clock = Math.Max(0, elapsed) - (StartMs ?? 0);
        if (StartMs.HasValue && clock < particle.BirthMs) return null;
        var age = ((clock - particle.BirthMs) % LoopMs + LoopMs) % LoopMs;
        if (age >= particle.LifespanMs) return null;
        var seconds = age / 1000; var progress = age / particle.LifespanMs;
        return new(particle.Position[0] / 1000 + Velocity[0] * seconds + Acceleration[0] * seconds * seconds / 2,
            particle.Position[1] / 1000 + Velocity[1] * seconds + Acceleration[1] * seconds * seconds / 2,
            particle.Scale / 1000 * Scale.Value(progress), ScaleX?.Value(progress) ?? 1, ScaleY?.Value(progress) ?? 1,
            Alpha.Value(progress), (particle.Angle + AngularSpeed * seconds) * Math.PI / 180);
    }
}
public sealed record MapParticle(double BirthMs, double LifespanMs, double[] Position, double Scale, double Angle);
public sealed record MapParticleState(double X, double Y, double Scale, double ScaleX, double ScaleY, double Alpha, double Angle);

/// <summary>Choice letters and conflicts follow the jointly obtainable engine match.</summary>
public static class ScoutChoices
{
    public static string Letter(int group) => ((char)('A' + group % 26)).ToString();
    public static IReadOnlyDictionary<int, ulong> Matched(IReadOnlyList<ScoutItem> items, IReadOnlySet<int> matches) =>
        items.Select((item, index) => (item, index)).Where(x => matches.Contains(x.index) && x.item.AccessibilityTag == 1)
            .GroupBy(x => x.item.AccessibilityGroup).ToDictionary(g => g.Key, g => g.Last().item.AccessibilityValue);
    public static bool Dimmed(ScoutItem item, bool matched, IReadOnlyDictionary<int, ulong> choices) =>
        !matched && item.AccessibilityTag == 1 && choices.TryGetValue(item.AccessibilityGroup, out var option) && option != item.AccessibilityValue;
}
