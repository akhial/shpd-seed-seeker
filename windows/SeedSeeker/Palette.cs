// SPDX-License-Identifier: GPL-3.0-or-later
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace SeedSeeker;

/// <summary>
/// The Shattered palette App.xaml defines, for views drawn in code, and the
/// tag every chip qualifier, stack count and Scout badge is drawn as, so the
/// board, Scout and the map's item card all share one look.
/// </summary>
internal static class Palette
{
    /// <summary>The game's yellow: qualifiers, counts, either/or capsules.</summary>
    public static Brush Amber => Get("ShpdAmberBrush");
    public static Brush AmberFill => Get("ShpdAmberFillBrush");
    public static Brush AmberFillHover => Get("ShpdAmberFillHoverBrush");
    public static Brush AmberStroke => Get("ShpdAmberStrokeBrush");
    public static Brush AmberWash => Get("ShpdAmberWashBrush");
    /// <summary>The game's upgrade green, on every "+N".</summary>
    public static Brush Upgrade => Get("ShpdUpgradeBrush");
    public static Brush UpgradeFill => Get("ShpdUpgradeFillBrush");
    /// <summary>The softer green of matches, applied trinkets and uncursed filters.</summary>
    public static Brush Green => Get("ShpdGreenBrush");
    public static Brush GreenFill => Get("ShpdGreenFillBrush");
    public static Brush GreenStroke => Get("ShpdGreenStrokeBrush");
    /// <summary>The faint wash behind a matched Scout row.</summary>
    public static Brush GreenWash => Get("ShpdGreenWashBrush");
    public static Brush Curse => Get("ShpdCurseBrush");
    public static Brush CurseFill => Get("ShpdCurseFillBrush");
    /// <summary>Problems and removal: the ink, and the solid fill white text sits on.</summary>
    public static Brush Danger => Get("ShpdDangerBrush");
    public static Brush DangerFill => Get("ShpdDangerFillBrush");
    /// <summary>Secrets and Arcane Resin.</summary>
    public static Brush Violet => Get("ShpdVioletBrush");
    public static Brush VioletFill => Get("ShpdVioletFillBrush");
    public static Brush VioletStroke => Get("ShpdVioletStrokeBrush");
    public static Brush VioletWash => Get("ShpdVioletWashBrush");
    /// <summary>Enchantments and glyphs named in text.</summary>
    public static Brush Teal => Get("ShpdTealBrush");
    /// <summary>A requirement chip's outline, and the dashed "+ Add" chip's.</summary>
    public static Brush ChipStroke => Get("ShpdChipStrokeBrush");
    /// <summary>A trinket card's outline, the web's faint violet.</summary>
    public static Brush TrinketStroke => Get("ShpdTrinketStrokeBrush");
    public static FontFamily Mono => (FontFamily)Application.Current.Resources["MonoFont"];

    public static Brush Get(string key) => (Brush)Application.Current.Resources[key];

    /// <summary>A tag: <paramref name="text"/> in the monospace pill of App.xaml's Tag style.</summary>
    public static Border Tag(string text, Brush ink, Brush fill) => new()
    {
        Style = (Style)Application.Current.Resources["Tag"], Background = fill,
        Child = new TextBlock { Text = text, Style = (Style)Application.Current.Resources["TagText"], Foreground = ink },
    };
}
