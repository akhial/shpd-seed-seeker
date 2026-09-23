// SPDX-License-Identifier: GPL-3.0-or-later
using System.Numerics;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.UI;

namespace SeedSeeker;

/// <summary>The web's faint category sprite, soft shadow, and green question mark.</summary>
internal sealed class WildcardSpriteView : Grid
{
    public WildcardSpriteView(ItemKind kind, double size = 18)
    {
        Width = Height = size;
        IsHitTestVisible = false;
        var index = kind switch
        {
            ItemKind.Weapon or ItemKind.MeleeWeapon => 112,
            ItemKind.ThrownWeapon => 149,
            ItemKind.Armor => 178,
            ItemKind.Wand => 209,
            ItemKind.Ring => 224,
            ItemKind.Trinket => 70,
            ItemKind.Artifact => 6,
            _ => 112,
        };
        Children.Add(new SpriteView { SpriteIndex = index, SpriteSize = size, Grayscale = true, Opacity = 0.3 });
        var shade = new RadialGradientBrush
        {
            Center = new Point(0.5, 0.5), GradientOrigin = new Point(0.5, 0.5), RadiusX = 0.5, RadiusY = 0.5,
        };
        shade.GradientStops.Add(new GradientStop { Offset = 0, Color = Color.FromArgb(102, 0, 0, 0) });
        shade.GradientStops.Add(new GradientStop { Offset = 0.45, Color = Color.FromArgb(46, 0, 0, 0) });
        shade.GradientStops.Add(new GradientStop { Offset = 1, Color = Colors.Transparent });
        Children.Add(new Border { Background = shade });
        var shadowHost = new Grid { HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
        Children.Add(shadowHost);
        var mark = new TextBlock
        {
            Text = "?", FontSize = size * 14 / 18, FontWeight = FontWeights.SemiBold,
            Foreground = new SolidColorBrush(Color.FromArgb(255, 131, 252, 100)),
            HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(mark, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        Children.Add(mark);
        void UpdateShadow()
        {
            var compositor = ElementCompositionPreview.GetElementVisual(mark).Compositor;
            var shadow = compositor.CreateDropShadow();
            shadow.Color = Colors.Black;
            shadow.BlurRadius = 2;
            shadow.Offset = new Vector3(0, 1, 0);
            shadow.Mask = mark.GetAlphaMask();
            var visual = compositor.CreateSpriteVisual();
            shadowHost.Width = mark.ActualWidth;
            shadowHost.Height = mark.ActualHeight;
            visual.Size = new Vector2((float)mark.ActualWidth, (float)mark.ActualHeight);
            visual.Shadow = shadow;
            ElementCompositionPreview.SetElementChildVisual(shadowHost, visual);
        }
        mark.Loaded += (_, _) => UpdateShadow();
        mark.SizeChanged += (_, _) => UpdateShadow();
    }
}
