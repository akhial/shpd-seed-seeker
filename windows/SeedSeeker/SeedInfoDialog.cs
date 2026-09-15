// SPDX-License-Identifier: GPL-3.0-or-later
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Text;

namespace SeedSeeker;

internal static class SeedInfoDialog
{
    public static async Task ShowAsync(XamlRoot root, string seed, ScoutItemMappings mappings)
    {
        var content = new StackPanel { Spacing = 14, Width = 321 };
        content.Children.Add(new TextBlock { Text = seed, FontFamily = new FontFamily("Cascadia Mono, Consolas"), FontSize = 18 });
        var detail = new TextBlock { TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
        content.Children.Add(detail);
        var artwork = ItemMappingArtwork.Shared;
        foreach (var (category, title, entries) in new[] {
            ("potions", "Potions", mappings.Potions), ("scrolls", "Scrolls", mappings.Scrolls), ("rings", "Rings", mappings.Rings),
        })
        {
            var group = new StackPanel { Spacing = 6 };
            group.Children.Add(new TextBlock { Text = title, FontWeight = FontWeights.SemiBold });
            var grid = new Grid { ColumnSpacing = artwork.SlotGap * 3, RowSpacing = artwork.SlotGap * 3 };
            for (var i = 0; i < 6; i++) grid.ColumnDefinitions.Add(new ColumnDefinition());
            for (var i = 0; i < 2; i++) grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            for (var index = 0; index < entries.Count; index++)
            {
                var entry = entries[index];
                var button = new Button {
                    Content = new MappingSprite(entry, artwork.Categories[category], index),
                    Padding = new Thickness(0), BorderThickness = new Thickness(0),
                    MinWidth = 0, MinHeight = 0,
                    Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                };
                foreach (var key in new[] { "ButtonBackground", "ButtonBackgroundPointerOver", "ButtonBackgroundPressed" })
                    button.Resources[key] = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
                AutomationProperties.SetName(button, entry.Label);
                ToolTipService.SetToolTip(button, entry.Label);
                button.Click += (_, _) => {
                    var show = detail.Visibility != Visibility.Visible || detail.Text != entry.Label;
                    detail.Text = entry.Label;
                    detail.Visibility = show ? Visibility.Visible : Visibility.Collapsed;
                };
                Grid.SetColumn(button, index % 6); Grid.SetRow(button, index / 6);
                grid.Children.Add(button);
            }
            group.Children.Add(grid); content.Children.Add(group);
        }
        var dialog = new ContentDialog {
            XamlRoot = root, Title = "Seed information", CloseButtonText = "Close",
            Content = new ScrollViewer { Content = content, VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled },
        };
        await dialog.ShowAsync();
    }

    private sealed class MappingSprite : Grid
    {
        private readonly Image image = new() { Stretch = Stretch.Fill };
        private readonly ScoutItemMapping entry;
        private readonly MappingArt art;
        private readonly int classIndex;
        private XamlRoot? attachedRoot;
        private int generation;

        public MappingSprite(ScoutItemMapping entry, MappingArt art, int classIndex)
        {
            this.entry = entry; this.art = art; this.classIndex = classIndex;
            Width = Height = ItemMappingArtwork.Shared.SlotSize * 3;
            Children.Add(image);
            IsHitTestVisible = false;
            Loaded += (_, _) => {
                attachedRoot = XamlRoot;
                if (attachedRoot is not null) attachedRoot.Changed += ScaleChanged;
                Render();
            };
            Unloaded += (_, _) => {
                generation++;
                if (attachedRoot is not null) attachedRoot.Changed -= ScaleChanged;
                attachedRoot = null;
            };
        }

        private void ScaleChanged(XamlRoot sender, XamlRootChangedEventArgs args) => Render();
        private async void Render()
        {
            var token = ++generation;
            var atlas = await ItemAtlas.GetAsync();
            if (token != generation || atlas is null) return;
            var size = (int)Math.Max(1, Math.Round(Width * (XamlRoot?.RasterizationScale ?? 1)));
            image.Source = atlas.MappingSprite(entry, art, classIndex, size);
        }
    }
}
