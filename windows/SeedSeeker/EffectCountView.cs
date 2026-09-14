// SPDX-License-Identifier: GPL-3.0-or-later
using System.Runtime.InteropServices.WindowsRuntime;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.UI;

namespace SeedSeeker;

/// <summary>Equal, stationary, smoothly blended effect colours around the count.</summary>
internal sealed class EffectCountView : Grid
{
    private readonly Color[] colors;
    private readonly Image ring = new() { Stretch = Stretch.Fill };
    private XamlRoot? root;
    public EffectCountView(IReadOnlyList<string> effects)
    {
        colors = effects.Select(effect => ItemGlow.ForEffect(effect)!.Color).ToArray();
        Width = Height = 18; VerticalAlignment = VerticalAlignment.Center;
        Children.Add(ring);
        Children.Add(new TextBlock { Text = effects.Count.ToString(), FontSize = 10, FontWeight = FontWeights.Bold,
            HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center });
        var label = $"{effects.Count} effects: {string.Join(", ", effects)}";
        ToolTipService.SetToolTip(this, label); AutomationProperties.SetName(this, label);
        Loaded += (_, _) => { root = XamlRoot; if (root is not null) root.Changed += RootChanged; Draw(); };
        Unloaded += (_, _) => { if (root is not null) root.Changed -= RootChanged; root = null; };
    }
    private void RootChanged(XamlRoot sender, XamlRootChangedEventArgs args) => Draw();
    private void Draw()
    {
        var size = Math.Max(1, (int)Math.Round(18 * (root?.RasterizationScale ?? 1)));
        var bytes = new byte[size * size * 4];
        var outer = size / 2d; var inner = size * 6.5 / 18;
        for (var y = 0; y < size; y++)
            for (var x = 0; x < size; x++)
            {
                var dx = x + .5 - outer; var dy = y + .5 - outer; var distance = Math.Sqrt(dx * dx + dy * dy);
                var alpha = Math.Clamp(Math.Min(outer - distance, distance - inner) + .5, 0, 1);
                var phase = ((Math.Atan2(dy, dx) + Math.PI / 2 + Math.PI * 2) % (Math.PI * 2)) / (Math.PI * 2) * colors.Length;
                var index = (int)phase; var mix = phase - index; var a = colors[index]; var b = colors[(index + 1) % colors.Length];
                var offset = (y * size + x) * 4;
                bytes[offset] = (byte)((a.B * (1 - mix) + b.B * mix) * alpha);
                bytes[offset + 1] = (byte)((a.G * (1 - mix) + b.G * mix) * alpha);
                bytes[offset + 2] = (byte)((a.R * (1 - mix) + b.R * mix) * alpha);
                bytes[offset + 3] = (byte)(alpha * 255);
            }
        var bitmap = new WriteableBitmap(size, size);
        using var stream = bitmap.PixelBuffer.AsStream(); stream.Write(bytes); bitmap.Invalidate(); ring.Source = bitmap;
    }
}
