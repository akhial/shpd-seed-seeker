using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace SeedSeeker;

/// <summary>The catalyst deck in its seeded order, using Fluent cards and pixel art.</summary>
public sealed class TrinketDeckView : StackPanel
{
    public Grid Choices { get; } = new() { ColumnSpacing = 6 };
    public TrinketDeckView(IReadOnlyList<CatalogItem> order, IReadOnlySet<string> matches, string? selectedTrinket, Action<string> onSelect)
    {
        Spacing = 10;
        Margin = new Thickness(0, 12, 0, 6);
        var choices = Choices;
        for (var i = 0; i < 4; i++) choices.ColumnDefinitions.Add(new ColumnDefinition());
        foreach (var (item, index) in order.Take(4).Select((item, index) => (item, index)))
        {
            var matched = matches.Contains(item.Id);
            var applied = selectedTrinket == item.Id;
            var card = new ToggleButton
            {
                CornerRadius = new CornerRadius(6), BorderThickness = new Thickness(applied ? 2 : 1),
                IsChecked = applied, Padding = new Thickness(0), HorizontalContentAlignment = HorizontalAlignment.Stretch,
                // Every card fills its quarter of the row, so all four are the
                // same square whatever their names (a toggle button hugs its content by default).
                HorizontalAlignment = HorizontalAlignment.Stretch, VerticalAlignment = VerticalAlignment.Top,
                VerticalContentAlignment = VerticalAlignment.Stretch,
                Background = applied ? Palette.GreenFill : Resource("CardBackgroundFillColorDefaultBrush"),
                BorderBrush = applied || matched ? Palette.Green : Palette.TrinketStroke,
            };
            StyleSelection(card);
            AutomationProperties.SetName(card, item.Name + (applied ? ", applied at +3" : "") + (matched ? ", matches requirement" : ""));
            card.Click += (_, _) => { card.IsChecked = applied; onSelect(applied ? "none" : item.Id); };
            ToolTipService.SetToolTip(card, item.Name);
            var body = new Grid { Padding = new Thickness(5) };
            body.RowDefinitions.Add(new RowDefinition { Height = new GridLength(16) });
            body.RowDefinitions.Add(new RowDefinition());
            body.RowDefinitions.Add(new RowDefinition { Height = new GridLength(20) });
            var sprite = new SpriteView { SpriteIndex = item.SpriteIndex, SpriteSize = 48,
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetRow(sprite, 1); body.Children.Add(sprite);
            var name = new Viewbox { Stretch = Stretch.Uniform, StretchDirection = StretchDirection.DownOnly,
                HorizontalAlignment = HorizontalAlignment.Stretch, Margin = new Thickness(0, 2, 0, 2),
                Child = new TextBlock { Text = item.Name, FontSize = 12, TextWrapping = TextWrapping.NoWrap } };
            Grid.SetRow(name, 2); body.Children.Add(name);
            if (applied)
            {
                var badge = Palette.Tag("Applied +3", Palette.Green, Palette.GreenFill);
                badge.HorizontalAlignment = HorizontalAlignment.Center; badge.VerticalAlignment = VerticalAlignment.Top;
                body.Children.Add(badge);
            }
            card.Content = body;
            card.SizeChanged += (_, _) =>
            {
                if (card.ActualWidth <= 0) return;
                card.Height = card.ActualWidth;
                sprite.SpriteSize = Math.Max(1, Math.Floor(Math.Min(48, Math.Min(card.ActualWidth - 12, card.ActualWidth - 50))));
            };
            Grid.SetColumn(card, index); choices.Children.Add(card);
        }
        Children.Add(choices);
        if (order.Count <= 4) return;
        Children.Add(new TextBlock { Text = "Transmutation order · 1–13", Style = (Style)Application.Current.Resources["Caption"] });
        var tail = new Grid { ColumnSpacing = 2 };
        foreach (var (item, index) in order.Skip(4).Select((item, index) => (item, index)))
        {
            tail.ColumnDefinitions.Add(new ColumnDefinition());
            var matched = matches.Contains(item.Id);
            var cell = new Border { Width = 24, Height = 24, CornerRadius = new CornerRadius(4),
                HorizontalAlignment = HorizontalAlignment.Center, BorderThickness = new Thickness(1),
                BorderBrush = matched ? Palette.Green : null,
                Background = matched ? Palette.GreenFill : null };
            var sprite = new SpriteView { SpriteIndex = item.SpriteIndex, SpriteSize = 20,
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
            cell.Child = sprite;
            var label = $"Transmutation #{index + 1}: {item.Name}" + (matched ? ", matches requirement" : "");
            ToolTipService.SetToolTip(cell, label); AutomationProperties.SetName(cell, label);
            tail.SizeChanged += (_, _) => { cell.Width = cell.Height = Math.Max(1, Math.Min(24, (tail.ActualWidth - 24) / 13)); sprite.SpriteSize = Math.Max(1, cell.Width - 4); };
            Grid.SetColumn(cell, index); tail.Children.Add(cell);
        }
        Children.Add(tail);
    }

    internal static void StyleSelection(ToggleButton button)
    {
        // Fluent's checked/hover visual states use these resources instead of
        // BorderBrush. Keep the applied colour green through each state, and
        // never paint an unselected hover as another applied choice.
        foreach (var state in new[] { "Checked", "CheckedPointerOver", "CheckedPressed", "CheckedDisabled" })
        {
            button.Resources["ToggleButtonBorderBrush" + state] = Palette.Green;
            button.Resources["ToggleButtonBackground" + state] = Palette.GreenFill;
            // The fill is a tint, not the accent, so the name keeps the body ink.
            button.Resources["ToggleButtonForeground" + state] = Resource(state == "CheckedDisabled" ? "TextFillColorDisabledBrush" : "TextFillColorPrimaryBrush");
        }
        foreach (var state in new[] { "PointerOver", "Pressed" })
        {
            button.Resources["ToggleButtonBorderBrush" + state] = button.BorderBrush;
            button.Resources["ToggleButtonBackground" + state] = button.Background;
        }
    }

    private static Brush Resource(string key) => (Brush)Application.Current.Resources[key];
}
