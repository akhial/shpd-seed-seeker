// SPDX-License-Identifier: GPL-3.0-or-later
namespace SeedSeeker;

/// <summary>Straight-alpha engine texture, decoded by the platform's PNG codec.</summary>
public sealed record MapTexture(int Width, int Height, byte[] Rgba);

/// <summary>
/// Software sprite compositor shared with host tests. Scenery changes only in
/// damaged cells; continuous particles and glow sample the monotonic display clock.
/// Output pixels are opaque BGRA in native little-endian memory order.
/// </summary>
public sealed class LevelMapRenderer
{
    private readonly LevelMapDocument map;
    private readonly IReadOnlyDictionary<string, MapTexture> textures;
    private readonly MapLayer[] layers;
    private readonly MapEmitter[] emitters;
    private readonly uint[] scenery, output, walls, darkness;
    private readonly HashSet<int> chasms;
    private readonly List<(MapLayer Layer, int Sprite)>[] cells;
    private readonly int[] frames;
    private readonly bool[] glowing;
    private readonly bool[] changed;
    private readonly MapEmitter[] wind, worldEffects, statusEffects;
    private int[] viewportSources = [];
    private (int Width, int Height, double Scale, double Left, double Top) viewportGeometry;
    private bool first = true;
    public int Width { get; }
    public int Height { get; }
    public bool Animated { get; }

    public LevelMapRenderer(LevelMapDocument document, IReadOnlyDictionary<string, MapTexture> images, bool secrets)
    {
        map = document; textures = images;
        Width = map.Width * 16; Height = map.Height * 16;
        layers = secrets ? map.Scene.Layers : map.Scene.ConcealedLayers;
        emitters = secrets ? map.Scene.Emitters : map.Scene.ConcealedEmitters;
        scenery = new uint[Width * Height]; output = new uint[Width * Height];
        walls = new uint[Width * Height]; darkness = new uint[Width * Height];
        cells = Enumerable.Range(0, map.Width * map.Height).Select(_ => new List<(MapLayer, int)>()).ToArray();
        foreach (var layer in layers)
            for (var cell = 0; cell < layer.Cells.Length; cell++)
                if (layer.Cells[cell] is int sprite) cells[cell].Add((layer, sprite));
        frames = Enumerable.Repeat(-1, map.Scene.Sprites.Length).ToArray();
        glowing = map.Scene.Sprites.Select(s => s.Frames.Any(f => f.Any(d => d.Glow is not null))).ToArray();
        changed = new bool[frames.Length];
        wind = emitters.Where(e => e.ClipToChasm).ToArray();
        worldEffects = emitters.Where(e => e.WallMask && !e.ClipToChasm).ToArray();
        statusEffects = emitters.Where(e => !e.WallMask && !e.ClipToChasm).ToArray();
        chasms = emitters.Where(e => e.ClipToChasm).Select(e => e.Cell).ToHashSet();
        Animated = emitters.Length > 0 || glowing.Any(g => g) || map.Scene.Sprites.Any(s => s.Frames.Length > 1);
        foreach (var layer in layers)
        {
            var mask = layer.Name == "darkness" ? darkness
                : layer.Name is "raised" or "walls" or "room_walls" or "boss_walls" ? walls : null;
            if (mask is null) continue;
            for (var cell = 0; cell < layer.Cells.Length; cell++)
                if (layer.Cells[cell] is int index)
                    foreach (var command in map.Scene.Sprites[index].Frames[0])
                        Draw(mask, command, cell % map.Width * 16, cell / map.Width * 16, false, 0);
        }
    }

    public uint[] Render(double elapsedMs)
    {
        UpdateScenery(elapsedMs);
        Array.Copy(scenery, output, scenery.Length);
        CompositeParticles(output, Width, Height, 1, 0, 0, elapsedMs, null);
        return output;
    }

    /// <summary>Nearest scenery and continuous particles at physical viewport resolution.</summary>
    public void RenderViewport(double elapsedMs, uint[] target, int width, int height, double scale, double left, double top)
    {
        UpdateScenery(elapsedMs);
        var geometry = (width, height, scale, left, top);
        if (viewportGeometry != geometry || viewportSources.Length != target.Length)
        {
            viewportGeometry = geometry;
            if (viewportSources.Length != target.Length) viewportSources = new int[target.Length];
            for (var y = 0; y < height; y++)
                for (var x = 0; x < width; x++)
                {
                    var sx = (int)Math.Floor((x + .5 - left) / scale); var sy = (int)Math.Floor((y + .5 - top) / scale);
                    viewportSources[y * width + x] = sx >= 0 && sx < Width && sy >= 0 && sy < Height ? sy * Width + sx : -1;
                }
        }
        for (var i = 0; i < target.Length; i++) target[i] = viewportSources[i] is >= 0 and var source ? scenery[source] : 0xff000000u;
        CompositeParticles(target, width, height, scale, left, top, elapsedMs, viewportSources);
    }

    private void UpdateScenery(double elapsedMs)
    {
        elapsedMs = Math.Max(0, elapsedMs);
        for (var i = 0; i < frames.Length; i++)
        {
            var sprite = map.Scene.Sprites[i];
            var frame = (int)(elapsedMs / sprite.FrameDurationMs % sprite.Frames.Length);
            changed[i] = frames[i] != frame || glowing[i]; frames[i] = frame;
        }
        for (var cell = 0; cell < cells.Length; cell++)
        {
            var dirty = first;
            if (!dirty) foreach (var entry in cells[cell]) { if (changed[entry.Sprite]) { dirty = true; break; } }
            if (!dirty) continue;
            var x = cell % map.Width * 16; var y = cell / map.Width * 16;
            for (var row = y; row < y + 16; row++) Array.Fill(scenery, 0xff000000u, row * Width + x, 16);
            foreach (var (layer, index) in cells[cell])
                foreach (var command in map.Scene.Sprites[index].Frames[frames[index]])
                    Draw(scenery, command, x, y, layer.Blend == "add", elapsedMs);
        }
        first = false;
    }

    private void CompositeParticles(uint[] target, int width, int height, double scale, double left, double top, double elapsedMs, int[]? sources)
    {
        if (emitters.Length == 0) return;
        // Chasm wind precedes the world effects and retains a union-of-cells clip.
        foreach (var emitter in wind) DrawEmitter(emitter, elapsedMs, target, width, height, scale, left, top);
        foreach (var emitter in worldEffects) DrawEmitter(emitter, elapsedMs, target, width, height, scale, left, top);
        RestoreMask(walls, target, sources);
        foreach (var emitter in statusEffects) DrawEmitter(emitter, elapsedMs, target, width, height, scale, left, top);
        RestoreMask(darkness, target, sources);
    }

    private void RestoreMask(uint[] mask, uint[] target, int[]? sources)
    {
        for (var i = 0; i < target.Length; i++)
        {
            var source = sources is null ? i : sources[i];
            if (source < 0) continue;
            var alpha = (int)(mask[source] >> 24);
            if (alpha == 0) continue;
            if (alpha == 255) { target[i] = scenery[source]; continue; }
            var a = target[i]; var b = scenery[source];
            target[i] = 0xff000000u | (uint)(((int)(a >> 16 & 255) * (255 - alpha) + (int)(b >> 16 & 255) * alpha) / 255) << 16
                | (uint)(((int)(a >> 8 & 255) * (255 - alpha) + (int)(b >> 8 & 255) * alpha) / 255) << 8
                | (uint)(((int)(a & 255) * (255 - alpha) + (int)(b & 255) * alpha) / 255);
        }
    }

    private void Draw(uint[] target, MapDraw command, double ox, double oy, bool add, double elapsed)
    {
        var d = command.Destination;
        var left = Math.Max(0, (int)Math.Floor(ox + d[0])); var top = Math.Max(0, (int)Math.Floor(oy + d[1]));
        var right = Math.Min(Width, (int)Math.Ceiling(ox + d[0] + d[2])); var bottom = Math.Min(Height, (int)Math.Ceiling(oy + d[1] + d[3]));
        for (var y = top; y < bottom; y++)
            for (var x = left; x < right; x++)
                Blend(target, y * Width + x, Sample(command, (x + .5 - ox - d[0]) / d[2], (y + .5 - oy - d[1]) / d[3], elapsed), 1, add);
    }

    private uint Sample(MapDraw command, double u, double v, double elapsed)
    {
        if (command.Kind == "fill") return (uint)(command.Rgba[3] << 24 | command.Rgba[0] << 16 | command.Rgba[1] << 8 | command.Rgba[2]);
        var texture = textures[command.Asset]; var s = command.Source;
        var x = Math.Clamp((int)(s[0] + u * s[2]), 0, texture.Width - 1);
        var y = Math.Clamp((int)(s[1] + v * s[3]), 0, texture.Height - 1);
        var offset = (y * texture.Width + x) * 4;
        var r = (int)texture.Rgba[offset]; var g = (int)texture.Rgba[offset + 1]; var b = (int)texture.Rgba[offset + 2];
        var a = texture.Rgba[offset + 3] * command.Opacity / 255;
        if (command.Tint is { } tint) { r = r * tint[0] / 255; g = g * tint[1] / 255; b = b * tint[2] / 255; }
        if (command.Glow is { PeriodMs: > 0 } glow)
        {
            var phase = elapsed / glow.PeriodMs % 2; var value = Math.Min(phase, 2 - phase) * .6;
            r = (int)Math.Round(r * (1 - value) + glow.Color[0] * value);
            g = (int)Math.Round(g * (1 - value) + glow.Color[1] * value);
            b = (int)Math.Round(b * (1 - value) + glow.Color[2] * value);
        }
        return (uint)(a << 24 | r << 16 | g << 8 | b);
    }

    private static void Blend(uint[] target, int index, uint color, double opacity, bool add)
    {
        var alpha = Math.Clamp((int)Math.Round((color >> 24) * opacity), 0, 255);
        if (alpha == 0) return;
        var old = target[index]; var inverse = add ? 255 : 255 - alpha;
        var r = Math.Min(255, ((int)(color >> 16 & 255) * alpha + (int)(old >> 16 & 255) * inverse) / 255);
        var g = Math.Min(255, ((int)(color >> 8 & 255) * alpha + (int)(old >> 8 & 255) * inverse) / 255);
        var b = Math.Min(255, ((int)(color & 255) * alpha + (int)(old & 255) * inverse) / 255);
        var a = Math.Min(255, alpha + (int)(old >> 24) * inverse / 255);
        target[index] = (uint)(a << 24 | r << 16 | g << 8 | b);
    }

    private void DrawEmitter(MapEmitter emitter, double elapsed, uint[] target, int targetWidth, int targetHeight, double density, double left, double top)
    {
        var width = emitter.Image.Destination[2]; var height = emitter.Image.Destination[3];
        foreach (var particle in emitter.Particles)
        {
            var state = emitter.State(particle, elapsed);
            if (state is null || state.Scale <= 0 || state.ScaleY <= 0 || state.Alpha <= 0) continue;
            var cx = left + (emitter.Cell % map.Width * 16 + state.X) * density; var cy = top + (emitter.Cell / map.Width * 16 + state.Y) * density;
            var cos = Math.Cos(state.Angle); var sin = Math.Sin(state.Angle);
            var halfWidth = width * state.Scale * density / 2; var halfHeight = height * state.Scale * state.ScaleY * density / 2;
            var radiusX = Math.Abs(cos) * halfWidth + Math.Abs(sin) * halfHeight;
            var radiusY = Math.Abs(sin) * halfWidth + Math.Abs(cos) * halfHeight;
            for (var y = Math.Max(0, (int)Math.Floor(cy - radiusY)); y < Math.Min(targetHeight, (int)Math.Ceiling(cy + radiusY)); y++)
                for (var x = Math.Max(0, (int)Math.Floor(cx - radiusX)); x < Math.Min(targetWidth, (int)Math.Ceiling(cx + radiusX)); x++)
                {
                    var sx = (int)Math.Floor((x + .5 - left) / density); var sy = (int)Math.Floor((y + .5 - top) / density);
                    if (sx < 0 || sy < 0 || sx >= Width || sy >= Height) continue;
                    if (emitter.ClipToChasm && !chasms.Contains(sy / 16 * map.Width + sx / 16)) continue;
                    var dx = x + .5 - cx; var dy = y + .5 - cy;
                    var u = (cos * dx + sin * dy) / (2 * halfWidth) + .5;
                    var v = (-sin * dx + cos * dy) / (2 * halfHeight) + .5;
                    if (u < 0 || u >= 1 || v < 0 || v >= 1) continue;
                    Blend(target, y * targetWidth + x, Sample(emitter.Image, u, v, elapsed), state.Alpha, emitter.Blend == "add");
                }
        }
    }
}
