using System.Text;
using System.Text.Json.Nodes;
using Xunit;

namespace SeedSeeker.Tests;

public sealed class LevelMapTests
{
    [Fact]
    public void ItemInspectionDecodesOriginalDescriptionsAndRejectsHiddenCellsAndMargins()
    {
        var map = NativeEngine.LevelMap(LevelMapDocument.Request("AAA-AAA-AAA", 1, 0, new QuerySettings(), "none"));
        var tip = map.ItemTooltips.First(tip => !tip.Hidden);
        Assert.NotEmpty(tip.Items[0].Description);
        Assert.NotNull(tip.Bounds);
        Assert.Contains(map.ItemTooltips.SelectMany(entry => entry.Items), item => item.Icon is { Length: 4 });
        var spriteX = tip.Cell % map.Width * 16 + tip.Bounds![0] + tip.Bounds[2] / 2.0;
        var spriteY = tip.Cell / map.Width * 16 + tip.Bounds[1];
        Assert.Same(tip, map.ItemAt(spriteX, spriteY + .5, false));
        Assert.NotSame(tip, map.ItemAt(spriteX, spriteY - .5, false));
        var x = tip.Cell % map.Width * 16 + 8;
        var y = tip.Cell / map.Width * 16 + 8;
        Assert.Same(tip, map.ItemAt(x, y, false));
        Assert.Null(map.ItemAt(-1, y, true));
        Assert.Null(map.ItemAt(map.Width * 16, y, true));
        var hidden = tip with { Hidden = true };
        var concealed = new LevelMapDocument { Width = map.Width, Height = map.Height, Scene = map.Scene, ItemTooltips = [hidden] };
        Assert.Null(concealed.ItemAt(x, y, false));
        Assert.Same(hidden, concealed.ItemAt(x, y, true));
    }

    [Fact]
    public void ItemInspectionDecodesGeneratedUpgradesEnchantmentsAndCurses()
    {
        var map = NativeEngine.LevelMap(LevelMapDocument.Request("AAA-AAA-AAA", 7, 0, new QuerySettings(), "none"));
        var items = map.ItemTooltips.SelectMany(tip => tip.Items).ToArray();
        var enchanted = items.First(item => item.Name == "Assassin's blade");
        Assert.Equal(1, enchanted.Upgrade); Assert.Equal("Vorpal", enchanted.Enchantment);
        Assert.False(enchanted.Cursed); Assert.Null(enchanted.Curse);
        Assert.Equal(new[] { 170, 102, 102 }, enchanted.Glow!.Color); Assert.Equal(1000, enchanted.Glow.PeriodMs);
        var cursed = items.First(item => item.Curse == "Wondrous");
        Assert.Equal(1, cursed.Upgrade); Assert.True(cursed.Cursed); Assert.Null(cursed.Enchantment);
        Assert.Equal(new[] { 0, 0, 0 }, cursed.Glow!.Color);
        Assert.Contains(items, item => item.Upgrade is null && item.Glow is null);
    }

    [Fact]
    public void GardenShaftsScaleWidthAndHeightIndependently()
    {
        var map = NativeEngine.LevelMap(LevelMapDocument.Request("AAA-AAA-AAA", 4, 0, new QuerySettings(), "none"));
        var shaft = map.Scene.Emitters.First(e => e.ScaleX is not null);
        var particle = shaft.Particles[0];
        var state = shaft.State(particle, particle.BirthMs + particle.LifespanMs / 2)!;
        Assert.Equal(2, state.ScaleX); Assert.Equal(24, state.ScaleY);
        var emitter = new MapEmitter { LoopMs = 1000,
            Image = new() { Kind = "fill", Destination = [0, 0, 1, 1], Rgba = [100, 0, 0, 255] },
            ScaleX = new([[0, 0], [1000, 4000]], false), ScaleY = new([[0, 16000], [1000, 32000]], false),
            Particles = [new(0, 1000, [8000, 8000], 1000, 0)] };
        var scene = Scene([new(1, [[Fill(0, 0, 0)]])], [new("terrain", null, [0])], [emitter]);
        var pixels = new LevelMapRenderer(scene, new Dictionary<string, MapTexture>(), true).Render(500);
        Assert.Equal(0xff640000u, pixels[7]); Assert.Equal(0xff640000u, pixels[15 * 16 + 8]);
        Assert.Equal(0xff000000u, pixels[8 * 16 + 6]); Assert.Equal(0xff000000u, pixels[8 * 16 + 9]);
    }
    private static MapDraw Fill(int r, int g, int b, int a = 255) => new() { Kind = "fill", Destination = [0, 0, 16, 16], Rgba = [r, g, b, a] };
    private static LevelMapDocument Scene(MapSprite[] sprites, MapLayer[] layers, MapEmitter[]? emitters = null, MapLayer[]? concealed = null) => new()
    {
        Width = 1, Height = 1, Scene = new() { TileSize = 16, Sprites = sprites, Layers = layers, ConcealedLayers = concealed ?? layers,
            Emitters = emitters ?? [], ConcealedEmitters = [] },
    };

    [Fact]
    public void SceneryUsesOrderedLayersAndConcealedAlternative()
    {
        var map = Scene([new(1, [[Fill(255, 0, 0)]]), new(1, [[Fill(0, 0, 255)]])],
            [new("terrain", null, [0]), new("custom_future_layer", null, [1])], concealed: [new("terrain", null, [0])]);
        Assert.Equal(0xff0000ffu, new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), true).Render(0)[0]);
        Assert.Equal(0xffff0000u, new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), false).Render(0)[0]);
    }

    [Fact]
    public void SpriteFramesWrapAndGlowChangesBetweenFrameBoundaries()
    {
        var draw = new MapDraw { Kind = "blit", Asset = "art", Source = [0, 0, 1, 1], Destination = [0, 0, 16, 16],
            Tint = [128, 255, 255], Opacity = 128, Glow = new([255, 0, 0], 1000) };
        var map = Scene([new(100, [[draw], [Fill(0, 255, 0)]])], [new("terrain", null, [0])]);
        var renderer = new LevelMapRenderer(map, new Dictionary<string, MapTexture> { ["art"] = new(1, 1, [100, 100, 100, 255]) }, true);
        var first = renderer.Render(0)[0];
        Assert.NotEqual(first, renderer.Render(50)[0]);
        Assert.Equal(0xff00ff00u, renderer.Render(100)[0]);
        Assert.Equal(first, renderer.Render(2000)[0]);
    }

    [Fact]
    public void AdditiveParticlesCompositeOntoSceneryAndRespectWalls()
    {
        var emitter = new MapEmitter { Cell = 0, LoopMs = 1000, Image = Fill(80, 0, 0), Blend = "add", WallMask = true,
            Particles = [new(0, 1000, [8000, 8000], 1000, 0)] };
        var map = Scene([new(1, [[Fill(20, 30, 40)]])], [new("terrain", null, [0])], [emitter]);
        Assert.Equal(0xff641e28u, new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), true).Render(0)[0]);
        var masked = Scene([new(1, [[Fill(20, 30, 40)]])], [new("walls", null, [0])], [emitter]);
        Assert.Equal(0xff141e28u, new LevelMapRenderer(masked, new Dictionary<string, MapTexture>(), true).Render(0)[0]);
        Assert.Equal(0xff141e28u, new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), false).Render(0)[0]);
    }

    [Fact]
    public void PartialWallMasksAttenuateWorldEffectsAndDarknessAlsoMasksStatus()
    {
        var emitter = new MapEmitter { Cell = 0, LoopMs = 1000, Image = Fill(80, 0, 0), Blend = "add", WallMask = true,
            Particles = [new(0, 1000, [8000, 8000], 1000, 0)] };
        var map = Scene([new(1, [[Fill(20, 30, 40)]]), new(1, [[Fill(20, 30, 40, 128)]])],
            [new("terrain", null, [0]), new("walls", null, [1])], [emitter]);
        Assert.Equal(0xff3b1e28u, new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), true).Render(0)[0]);
        var status = new MapEmitter { LoopMs = 1000, Image = Fill(80, 0, 0), Particles = emitter.Particles };
        var dark = Scene([new(1, [[Fill(0, 0, 0)]])], [new("darkness", null, [0])], [status]);
        Assert.Equal(0xff000000u, new LevelMapRenderer(dark, new Dictionary<string, MapTexture>(), true).Render(0)[0]);
    }

    [Fact]
    public void WindCanCrossAdjacentChasmsButCannotSpillOntoFloor()
    {
        var emitter = new MapEmitter { LoopMs = 1000, ClipToChasm = true,
            Image = new() { Kind = "fill", Destination = [0, 0, 4, 4], Rgba = [255, 255, 255, 255] },
            Particles = [new(0, 1000, [16000, 8000], 1000, 0)] };
        LevelMapDocument Map(MapEmitter[] emitters) => new() { Width = 2, Height = 1,
            Scene = new() { TileSize = 16, Sprites = [new(1, [[Fill(0, 0, 0)]])], Layers = [new("terrain", null, [0, 0])], Emitters = emitters } };
        var isolated = new LevelMapRenderer(Map([emitter]), new Dictionary<string, MapTexture>(), true).Render(0);
        Assert.Equal(0xffffffffu, isolated[8 * 32 + 15]); Assert.Equal(0xff000000u, isolated[8 * 32 + 16]);
        var connected = new LevelMapRenderer(Map([emitter, new() { Cell = 1, ClipToChasm = true, LoopMs = 1000, Image = emitter.Image }]), new Dictionary<string, MapTexture>(), true).Render(0);
        Assert.Equal(0xffffffffu, connected[8 * 32 + 16]);
    }

    [Fact]
    public void ScheduledHazardsDoNotPrewarmAndCurvesInterpolate()
    {
        var emitter = new MapEmitter { StartMs = 1000, LoopMs = 2000, Velocity = [4, 0], Acceleration = [0, 2],
            Scale = new([[0, 0], [1000, 1000]], true), ScaleY = new([[0, 1000], [1000, 0]], false) };
        var particle = new MapParticle(200, 1000, [0, 0], 2000, 10);
        Assert.Null(emitter.State(particle, 0));
        Assert.Null(emitter.State(particle, 1199));
        var state = Assert.IsType<MapParticleState>(emitter.State(particle, 1700));
        Assert.Equal(2, state.X); Assert.Equal(.25, state.Y); Assert.Equal(Math.Sqrt(.5) * 2, state.Scale);
        Assert.Equal(.5, state.ScaleY);
        Assert.Null(emitter.State(particle, 2200));
        Assert.Equal(emitter.State(particle, 1200), emitter.State(particle, 3200));
    }

    [Fact]
    public void ZoomedParticlesAdvanceAtPhysicalPixelResolution()
    {
        var emitter = new MapEmitter { LoopMs = 1000, Velocity = [1, 0],
            Image = new() { Kind = "fill", Destination = [0, 0, 2, 2], Rgba = [255, 255, 255, 255] },
            Particles = [new(0, 1000, [8000, 8000], 1000, 0)] };
        var map = Scene([new(1, [[Fill(0, 0, 0)]])], [new("terrain", null, [0])], [emitter]);
        var renderer = new LevelMapRenderer(map, new Dictionary<string, MapTexture>(), true);
        var pixels = new uint[64 * 64];
        renderer.RenderViewport(0, pixels, 64, 64, 4, 0, 0);
        Assert.Equal(0xffffffffu, pixels[32 * 64 + 28]);
        renderer.RenderViewport(250, pixels, 64, 64, 4, 0, 0);
        Assert.Equal(0xff000000u, pixels[32 * 64 + 28]);
        Assert.Equal(0xffffffffu, pixels[32 * 64 + 29]);
    }

    [Fact]
    public void UnknownSchemaIsRejectedBeforeRendering()
    {
        Assert.Throws<InvalidDataException>(() => LevelMapDocument.Parse(Encoding.UTF8.GetBytes("""{"format":"seed-seeker-level-map","schemaVersion":4}""")));
    }

    [Theory]
    [InlineData(1, 0)]
    [InlineData(5, 0)]
    [InlineData(15, 0)]
    [InlineData(17, 1)]
    public void NativeMapSupportsPublishedFloorsAndBranch(int depth, int branch)
    {
        var request = LevelMapDocument.Request("AAA-AAA-BUH", depth, branch, new QuerySettings(), "none");
        var map = NativeEngine.LevelMap(request);
        Assert.Equal(3, map.SchemaVersion); Assert.Equal(depth, map.Depth); Assert.Equal(branch, map.Branch);
        Assert.NotEmpty(map.Scene.Sprites); Assert.NotEmpty(map.Assets);
        Assert.Contains(depth, EngineInfo.MapDepths);
        var asset = map.Assets[0]; var bytes = NativeEngine.LevelMapAsset(asset.Id);
        Assert.Equal(asset.Sha256, Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(bytes)).ToLowerInvariant());
    }

    [Fact]
    public void MapRequestUsesScoutChallengeAndTrinketProfile()
    {
        var query = new QuerySettings { Challenges = 264, Requirements = [new() { Kind = ItemKind.Trinket, Item = ItemCatalog.Find("mimic_tooth"), SelectTrinket = true }] };
        var request = JsonNode.Parse(LevelMapDocument.Request("AAA-AAA-AAA", 15, 1, query, "none"))!;
        Assert.Equal("none", (string?)request["trinket"]); Assert.Equal(1, (int?)request["branch"]);
        Assert.Equal(new[] { "barren_land", "badder_bosses" }, request["challenges"]!.AsArray().Select(x => (string)x!).ToArray());
        Assert.NotNull(request["query"]);
        Assert.DoesNotContain(10, EngineInfo.MapDepths); Assert.DoesNotContain(20, EngineInfo.MapDepths);
    }

    [Fact]
    public void MatchingChoiceDimsOnlyConflictingAlternatives()
    {
        var item = new ScoutItem(ItemCatalog.Find("wand_frost")!, 1, 0, null, false, ScoutItemSource.CrystalChest, 1, 0, 0);
        ScoutItem[] items = [item, item with { AccessibilityValue = 1 }, item with { AccessibilityGroup = 1 }, item with { AccessibilityTag = 2 }];
        var choices = ScoutChoices.Matched(items, new HashSet<int> { 0 });
        Assert.False(ScoutChoices.Dimmed(items[0], true, choices));
        Assert.True(ScoutChoices.Dimmed(items[1], false, choices));
        Assert.False(ScoutChoices.Dimmed(items[2], false, choices));
        Assert.False(ScoutChoices.Dimmed(items[3], false, choices));
        Assert.Equal("A", ScoutChoices.Letter(0)); Assert.Equal("B", ScoutChoices.Letter(27));
    }
}
