// SPDX-License-Identifier: GPL-3.0-or-later
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Security.Cryptography;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Foundation;
using Windows.Graphics.Imaging;
using Windows.System;
using Windows.UI.ViewManagement;

namespace SeedSeeker;

internal sealed record MapBundle(LevelMapDocument Map, IReadOnlyDictionary<string, MapTexture> Textures);

internal static class LevelMapCache
{
    private static readonly Dictionary<string, Task<MapBundle>> Maps = [];
    private static readonly Dictionary<string, Task<MapTexture>> Art = [];
    private static readonly SemaphoreSlim Generation = new(1);

    // Accessed on the UI thread. Tasks share in-flight work; failed work is evicted.
    public static async Task<MapBundle> Get(string request)
    {
        var key = EngineInfo.MapRevision + "/" + request;
        if (!Maps.TryGetValue(key, out var task))
        {
            if (Maps.Count >= 24) Maps.Remove(Maps.Keys.First());
            Maps[key] = task = Load(request);
        }
        try { return await task; }
        catch { if (Maps.GetValueOrDefault(key) == task) Maps.Remove(key); throw; }
    }

    private static async Task<MapBundle> Load(string request)
    {
        await Generation.WaitAsync();
        LevelMapDocument map;
        try { map = await Task.Run(() => NativeEngine.LevelMap(request)); }
        finally { Generation.Release(); }
        var textures = new Dictionary<string, MapTexture>();
        foreach (var asset in map.Assets)
        {
            var key = map.AssetRevision + "/" + asset.Id + "/" + asset.Sha256;
            if (!Art.TryGetValue(key, out var task)) Art[key] = task = Decode(asset);
            try { textures[asset.Id] = await task; }
            catch { Art.Remove(key); throw; }
        }
        return new(map, textures);
    }

    private static async Task<MapTexture> Decode(MapAsset asset)
    {
        var png = await Task.Run(() => NativeEngine.LevelMapAsset(asset.Id));
        if (!Convert.ToHexString(SHA256.HashData(png)).Equals(asset.Sha256, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("The engine's map art revision does not match the scene.");
        using var memory = new MemoryStream(png);
        using var stream = memory.AsRandomAccessStream();
        var decoder = await BitmapDecoder.CreateAsync(stream);
        if (decoder.PixelWidth != asset.Width || decoder.PixelHeight != asset.Height)
            throw new InvalidDataException("The engine's map art has unexpected dimensions.");
        var pixels = await decoder.GetPixelDataAsync(BitmapPixelFormat.Rgba8, BitmapAlphaMode.Straight,
            new BitmapTransform(), ExifOrientationMode.IgnoreExifOrientation, ColorManagementMode.DoNotColorManage);
        return new(asset.Width, asset.Height, pixels.DetachPixelData());
    }
}

/// <summary>One scout profile survives manifest replacement during trinket changes.</summary>
internal sealed class LevelMapSession(ScoutWorld world, QuerySettings query, Func<string, Task> selectTrinket)
{
    public ScoutWorld World { get; private set; } = world;
    public QuerySettings Query { get; private set; } = query;
    public bool Secrets { get; set; }
    public bool Busy { get; private set; }
    public event Action? Changed;
    public int[] Floors => EngineInfo.MapDepths.Where(depth => depth <= Math.Max(
        World.Items.Select(item => item.Depth).DefaultIfEmpty(0).Max(),
        World.FloorFeelings?.Select(feeling => feeling.Depth).DefaultIfEmpty(0).Max() ?? 0)).ToArray();
    public void Update(ScoutWorld next, QuerySettings profile) { World = next; Query = profile; Changed?.Invoke(); }
    public async Task Select(string id)
    {
        if (Busy) return;
        Busy = true; Changed?.Invoke();
        try { await selectTrinket(id); }
        finally { Busy = false; Changed?.Invoke(); }
    }
}

/// <summary>Fluent controls around a native scene; no browser or external assets.</summary>
internal sealed class LevelMapView : Grid
{
    private readonly LevelMapSession session;
    private readonly bool expanded;
    private readonly WrapPanel toolbar = new() { Spacing = 6, LineSpacing = 6 };
    private readonly StackPanel areas = new() { Orientation = Orientation.Horizontal, Spacing = 4 };
    private readonly ComboBox floors = new() { MinWidth = 140 };
    private readonly ComboBox trinkets = new() { MinWidth = 140, MaxWidth = 220 };
    private readonly TextBlock identity = new() { TextWrapping = TextWrapping.Wrap, Opacity = .75 };
    private readonly Button previous = new() { Content = "Previous" };
    private readonly Button next = new() { Content = "Next" };
    private readonly ToggleButton secrets = new() { Content = "Secrets", IsEnabled = false };
    private readonly Grid stage = new() { Background = new SolidColorBrush(Microsoft.UI.Colors.Black) };
    private readonly ContentControl stageHost = new() { IsTabStop = true, HorizontalContentAlignment = HorizontalAlignment.Stretch, VerticalContentAlignment = VerticalAlignment.Stretch };
    private readonly Image art = new() { Stretch = Stretch.Fill };
    private readonly StackPanel status = new() { Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    private readonly TextBlock message = new() { TextWrapping = TextWrapping.Wrap, MaxWidth = 400, Foreground = new SolidColorBrush(Microsoft.UI.Colors.White) };
    private readonly Button retry = new() { Content = "Try again", HorizontalAlignment = HorizontalAlignment.Center };
    private readonly Stopwatch clock = new();
    private readonly UISettings settings = new();
    private LevelMapRenderer? renderer;
    private MapBundle? bundle;
    private MapBranch[] branches = [];
    private WriteableBitmap? bitmap;
    private uint[] viewport = [];
    private int depth, branch, generation;
    private bool live, visible = true, suspended, rendering, updating;
    private string? requestKey;
    private string? profileLocationKey, viewportLocationKey;
    private double zoom = 1, panX, panY;
    private Point? pointer;
    private Border? itemCard;
    private int? inspectedCell;
    private void HideItem() { if (itemCard is not null) stage.Children.Remove(itemCard); itemCard = null; inspectedCell = null; }
    private void InspectItem(Point point)
    {
        if (bundle is null || Fit * zoom <= 0) { HideItem(); return; }
        if (itemCard is not null && new Rect(itemCard.Margin.Left - 18, itemCard.Margin.Top - 18,
            itemCard.ActualWidth + 36, itemCard.ActualHeight + 36).Contains(point)) return;
        var map = bundle.Map; var scale = Fit * zoom;
        var tip = map.ItemAt((point.X - stage.ActualWidth / 2 - panX) / scale + map.Width * map.Scene.TileSize / 2d,
            (point.Y - stage.ActualHeight / 2 - panY) / scale + map.Height * map.Scene.TileSize / 2d, session.Secrets);
        if (tip?.Cell == inspectedCell) return;
        HideItem();
        if (tip is null) return;
        inspectedCell = tip.Cell;
        var body = new StackPanel { Spacing = 10 };
        if (tip.Label.Length > 0) body.Children.Add(new TextBlock { Text = tip.Label, FontSize = 11, Opacity = .7, Margin = new Thickness(0, 0, 0, 4) });
        foreach (var item in tip.Items)
        {
            var heading = new Grid { ColumnSpacing = 10 };
            heading.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
            heading.ColumnDefinitions.Add(new() { Width = new GridLength(1, GridUnitType.Star) });
            var sprite = new SpriteView { SpriteIndex = item.Image, IconSource = item.Icon, AlignArtworkLeft = true, SpriteSize = 32, VerticalAlignment = VerticalAlignment.Center };
            if (item.Glow is { Color.Length: 3 } glow) {
                sprite.GlowColor = Windows.UI.Color.FromArgb(255, (byte)glow.Color[0], (byte)glow.Color[1], (byte)glow.Color[2]);
                sprite.GlowPeriod = glow.PeriodMs / 1000;
            }
            heading.Children.Add(sprite);
            var name = new TextBlock { Text = item.Name + (item.Quantity > 1 ? $"  ×{item.Quantity}" : ""),
                FontSize = 16, FontWeight = Microsoft.UI.Text.FontWeights.Bold, TextWrapping = TextWrapping.Wrap, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(name, 1); heading.Children.Add(name);
            if (item.Upgrade is > 0 and int upgrade) {
                heading.ColumnDefinitions.Add(new() { Width = GridLength.Auto });
                var chip = new Border { Padding = new Thickness(4, 0, 4, 0), CornerRadius = new CornerRadius(4),
                    VerticalAlignment = VerticalAlignment.Center,
                    Background = (Brush)Application.Current.Resources["SystemFillColorSuccessBackgroundBrush"],
                    Child = new TextBlock { Text = $"+{upgrade}", FontSize = 11, FontFamily = new FontFamily("Consolas"),
                        FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                        Foreground = (Brush)Application.Current.Resources["SystemFillColorSuccessBrush"] } };
                AutomationProperties.SetName(chip, $"Upgrade +{upgrade}");
                Grid.SetColumn(chip, 2); heading.Children.Add(chip);
            }
            body.Children.Add(heading);
            var modifiers = new WrapPanel { Spacing = 6, LineSpacing = 4 };
            if (item.Cursed || item.Curse is not null) modifiers.Children.Add(new TextBlock {
                Text = item.Cursed ? "Cursed" : "Curse", FontSize = 12,
                Foreground = (Brush)Application.Current.Resources["SystemFillColorCriticalBrush"] });
            if (modifiers.Children.Count > 0) body.Children.Add(modifiers);
            if (!item.Deterministic) body.Children.Add(new TextBlock { Text = "Varies with play", FontSize = 11, Opacity = .7 });
            if (item.Description.Length > 0) body.Children.Add(new TextBlock { Text = item.Description, FontSize = 12, Opacity = .85, TextWrapping = TextWrapping.Wrap });
        }
        var cardWidth = Math.Max(1, Math.Min(310, stage.ActualWidth - 16));
        itemCard = new Border {
            Width = cardWidth, MaxHeight = Math.Max(1, Math.Min(320, stage.ActualHeight - 16)), Padding = new Thickness(14),
            CornerRadius = new CornerRadius(0), BorderThickness = new Thickness(1),
            Background = (Brush)Application.Current.Resources["SolidBackgroundFillColorBaseBrush"],
            BorderBrush = (Brush)Application.Current.Resources["CardStrokeColorDefaultBrush"],
            HorizontalAlignment = HorizontalAlignment.Left, VerticalAlignment = VerticalAlignment.Top,
            Child = new ScrollViewer { Content = body, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
        };
        itemCard.PointerPressed += (_, e) => e.Handled = true;
        itemCard.PointerWheelChanged += (_, e) => e.Handled = true;
        itemCard.Tapped += (_, e) => e.Handled = true;
        itemCard.Measure(new Size(cardWidth, itemCard.MaxHeight));
        var height = itemCard.DesiredSize.Height;
        itemCard.Margin = new Thickness(Math.Max(8, Math.Min(point.X + 16, stage.ActualWidth - cardWidth - 8)),
            Math.Max(8, Math.Min(point.Y + 16 + height < stage.ActualHeight ? point.Y + 16 : point.Y - height - 16, stage.ActualHeight - height - 8)), 0, 0);
        AutomationProperties.SetName(itemCard, string.Join(", ", tip.Items.Select(item => item.Name)));
        stage.Children.Add(itemCard);
    }
    private bool subscribed;
    private bool gestureAtFit;
    private bool? lastMotion;
    public LevelMapView(LevelMapSession profile, int floor, bool isExpanded = false)
    {
        session = profile; depth = floor; expanded = isExpanded;
        RowDefinitions.Add(new() { Height = GridLength.Auto });
        RowDefinitions.Add(new() { Height = new GridLength(1, GridUnitType.Star) });
        RowSpacing = 8;
        if (!expanded) Height = 390;
        toolbar.Children.Add(areas); toolbar.Children.Add(secrets);
        AddButton("−", () => Zoom(zoom / 1.5, new Point()), "Zoom out (−)");
        AddButton("+", () => Zoom(zoom * 1.5, new Point()), "Zoom in (+)");
        AddButton("Fit", Reset, "Fit map (0)");
        if (!expanded) AddButton("Expand", () => _ = Expand(), "Expand floor map");
        var header = new StackPanel { Spacing = 8 };
        if (expanded)
        {
            var navigation = new WrapPanel { Spacing = 8, LineSpacing = 6 };
            navigation.Children.Add(floors); navigation.Children.Add(trinkets);
            previous.Click += (_, _) => Navigate(-1); next.Click += (_, _) => Navigate(1);
            ToolTipService.SetToolTip(previous, "Previous floor (K)"); ToolTipService.SetToolTip(next, "Next floor (J)");
            navigation.Children.Add(previous); navigation.Children.Add(next); header.Children.Add(navigation); header.Children.Add(identity);
            floors.ItemsSource = session.Floors.Select(d => $"Floor {d}").ToArray();
            floors.SelectedIndex = Array.IndexOf(session.Floors, depth);
            floors.SelectionChanged += (_, _) => { if (!updating && floors.SelectedIndex >= 0) ChangeFloor(session.Floors[floors.SelectedIndex]); };
            trinkets.SelectionChanged += async (_, _) =>
            {
                if (updating || trinkets.SelectedIndex < 0) return;
                var offered = session.World.TrinketOrder?.Take(4).ToArray() ?? [];
                var selected = trinkets.SelectedIndex == 0 ? "none" : offered[trinkets.SelectedIndex - 1].Id;
                await session.Select(selected);
            };
            AutomationProperties.SetName(floors, "Floor"); AutomationProperties.SetName(trinkets, "Applied trinket");
        }
        header.Children.Add(toolbar); Children.Add(header);
        SetRow(stageHost, 1); stage.Children.Add(art); stage.Children.Add(status); stageHost.Content = stage; Children.Add(stageHost);
        status.Children.Add(message); status.Children.Add(retry);
        retry.Click += (_, _) => { requestKey = null; _ = Load(); };
        secrets.Click += (_, _) =>
        {
            HideItem();
            session.Secrets = secrets.IsChecked == true;
            if (bundle is not null) renderer = new(bundle.Map, bundle.Textures, session.Secrets);
            UpdateToolbar(); Render();
        };
        stage.SizeChanged += (_, _) => { HideItem(); Constrain(); Render(); };
        stage.PointerExited += (_, e) => {
            var point = e.GetCurrentPoint(stage).Position;
            if (point.X < 0 || point.Y < 0 || point.X >= stage.ActualWidth || point.Y >= stage.ActualHeight) HideItem();
        };
        stage.Tapped += (_, e) => InspectItem(e.GetPosition(stage));
        stage.PointerWheelChanged += (_, e) =>
        {
            var point = e.GetCurrentPoint(stage);
            Zoom(zoom * Math.Pow(1.0018, point.Properties.MouseWheelDelta), new(point.Position.X - stage.ActualWidth / 2, point.Position.Y - stage.ActualHeight / 2));
            e.Handled = true;
        };
        stage.PointerPressed += (_, e) =>
        {
            HideItem();
            var current = e.GetCurrentPoint(stage);
            if (e.Pointer.PointerDeviceType != Microsoft.UI.Input.PointerDeviceType.Mouse || !current.Properties.IsLeftButtonPressed) return;
            stageHost.Focus(FocusState.Pointer);
            pointer = current.Position; stage.CapturePointer(e.Pointer); e.Handled = true;
        };
        stage.PointerMoved += (_, e) =>
        {
            if (pointer is not Point old) {
                var current = e.GetCurrentPoint(stage);
                if (!current.IsInContact) InspectItem(current.Position);
                return;
            }
            HideItem();
            var point = e.GetCurrentPoint(stage).Position; panX += point.X - old.X; panY += point.Y - old.Y; pointer = point;
            Constrain(); Render(); e.Handled = true;
        };
        stage.PointerReleased += (_, e) => { pointer = null; stage.ReleasePointerCapture(e.Pointer); };
        stage.PointerCaptureLost += (_, _) => pointer = null;
        stage.ManipulationMode = ManipulationModes.Scale | ManipulationModes.TranslateX | ManipulationModes.TranslateY;
        stage.ManipulationStarted += (_, _) => gestureAtFit = zoom <= 1.001;
        stage.ManipulationDelta += (_, e) =>
        {
            Zoom(zoom * e.Delta.Scale, new(e.Position.X - stage.ActualWidth / 2, e.Position.Y - stage.ActualHeight / 2));
            panX += e.Delta.Translation.X; panY += e.Delta.Translation.Y; Constrain(); Render(); e.Handled = true;
        };
        stage.ManipulationCompleted += (_, e) =>
        {
            var movement = e.Cumulative.Translation;
            if (expanded && gestureAtFit && zoom <= 1.001 && Math.Abs(movement.X) >= 60 && Math.Abs(movement.X) >= Math.Abs(movement.Y) * 1.5)
                Navigate(movement.X < 0 ? 1 : -1);
        };
        KeyDown += OnKeyDown;
        Loaded += (_, _) =>
        {
            live = true; session.Changed += ProfileChanged;
            if (XamlRoot is not null) XamlRoot.Changed += RootChanged;
            ProfileChanged(); _ = Load(); SyncAnimation();
        };
        Unloaded += (_, _) =>
        {
            HideItem();
            live = false; ++generation; requestKey = null; session.Changed -= ProfileChanged;
            if (XamlRoot is not null) XamlRoot.Changed -= RootChanged;
            SyncAnimation();
        };
        EffectiveViewportChanged += (sender, e) =>
        {
            var viewport = e.EffectiveViewport;
            visible = viewport.Width > 0 && viewport.Height > 0 && viewport.Right > 0 && viewport.Bottom > 0
                && viewport.Left < ActualWidth && viewport.Top < ActualHeight;
            if (visible) _ = Load(); SyncAnimation();
        };
        AutomationProperties.SetName(stage, $"Floor {depth} map. Mouse wheel or pinch to zoom, drag to pan.");
    }

    private void RootChanged(XamlRoot sender, XamlRootChangedEventArgs args) { SyncAnimation(); Render(); }
    private void AddButton(string title, Action action, string help)
    {
        var button = new Button { Content = title, Padding = new Thickness(10, 5, 10, 5) };
        button.Click += (_, _) => action(); ToolTipService.SetToolTip(button, help); AutomationProperties.SetName(button, help); toolbar.Children.Add(button);
    }
    private void ProfileChanged()
    {
        updating = true;
        var offered = session.World.TrinketOrder?.Take(4).ToArray() ?? [];
        trinkets.ItemsSource = new[] { "No trinket" }.Concat(offered.Select(item => item.Name)).ToArray();
        trinkets.SelectedIndex = session.World.SelectedTrinket is null ? 0 : Array.FindIndex(offered, item => item.Id == session.World.SelectedTrinket) + 1;
        trinkets.IsEnabled = !session.Busy;
        updating = false;
        var next = LevelMapDocument.Request(session.World.Seed, depth, 0, session.Query, session.World.SelectedTrinket ?? "none");
        var location = LevelMapDocument.Request(session.World.Seed, depth, 0, session.Query, "none");
        if (profileLocationKey != location) { branch = 0; branches = []; }
        profileLocationKey = location;
        if (profileKey != next) { profileKey = next; requestKey = null; _ = Load(); }
    }
    private string? profileKey;
    private async Task Load()
    {
        if (!live || !visible || suspended) return;
        var request = LevelMapDocument.Request(session.World.Seed, depth, branch, session.Query, session.World.SelectedTrinket ?? "none");
        if (requestKey == request) return;
        HideItem();
        requestKey = request; var token = ++generation; bundle = null; renderer = null; art.Source = null;
        var location = LevelMapDocument.Request(session.World.Seed, depth, branch, session.Query, "none");
        if (viewportLocationKey != location) { zoom = 1; panX = panY = 0; }
        viewportLocationKey = location;
        profileLocationKey = LevelMapDocument.Request(session.World.Seed, depth, 0, session.Query, "none");
        var parentRequest = LevelMapDocument.Request(session.World.Seed, depth, 0, session.Query, session.World.SelectedTrinket ?? "none");
        profileKey = parentRequest;
        message.Text = $"Charting floor {depth}…"; status.Visibility = Visibility.Visible; retry.Visibility = Visibility.Collapsed;
        SyncAnimation(); UpdateToolbar();
        try
        {
            var main = await LevelMapCache.Get(parentRequest);
            if (token != generation || !live) return;
            branches = main.Map.Branches;
            if (branch != 0 && !branches.Any(area => area.Branch == branch))
            {
                branch = 0; requestKey = null; await Load(); return;
            }
            var loaded = branch == 0 ? main : await LevelMapCache.Get(request);
            if (token != generation || !live) return;
            bundle = loaded;
            renderer = new(loaded.Map, loaded.Textures, session.Secrets); clock.Restart();
            status.Visibility = Visibility.Collapsed; UpdateToolbar(); Constrain(); Render(); SyncAnimation();
        }
        catch (Exception ex)
        {
            if (token != generation || !live) return;
            message.Text = $"Couldn’t load this map. {ex.Message}"; retry.Visibility = Visibility.Visible;
        }
    }
    private void UpdateToolbar()
    {
        previous.IsEnabled = depth != session.Floors.FirstOrDefault(); next.IsEnabled = depth != session.Floors.LastOrDefault();
        var region = depth switch { <= 5 => "Sewers", <= 10 => "Prison", <= 15 => "Caves", <= 20 => "Dwarven City", _ => "Demon Halls" };
        var feeling = session.World.FloorFeelings?.FirstOrDefault(f => f.Depth == depth)?.Feeling ?? FloorFeeling.None;
        var quest = session.World.Quests.FirstOrDefault(q => q.Depth == depth);
        identity.Text = string.Join(" · ", new[] { region, feeling == FloorFeeling.None ? "" : $"{feeling} floor",
            quest is null ? "" : ScoutQuests.VariantLabel(quest.Variant) }.Where(text => text.Length > 0));
        secrets.IsChecked = session.Secrets; secrets.IsEnabled = bundle?.Map.HasSecrets == true;
        secrets.Content = session.Secrets ? "✓ Secrets" : "Secrets";
        ToolTipService.SetToolTip(secrets, bundle?.Map.HasSecrets == false ? "No secrets on this map" : "Reveal secret rooms, doors and traps");
        areas.Children.Clear();
        if (branches.Length == 0) return;
        foreach (var area in new[] { (Branch: 0, Label: "Main") }.Concat(branches.Select(b => (b.Branch, Label: b.Kind == "imp_vault" ? "Imp Vault" : "Blacksmith Mine"))))
        {
            var button = new ToggleButton { Content = area.Label, IsChecked = area.Branch == branch, Padding = new Thickness(8, 5, 8, 5) };
            button.Click += (_, _) => { branch = area.Branch; _ = Load(); }; areas.Children.Add(button);
        }
    }
    private void ChangeFloor(int next)
    {
        if (depth == next) return;
        depth = next; branch = 0; branches = []; requestKey = null;
        updating = true; floors.SelectedIndex = Array.IndexOf(session.Floors, depth); updating = false;
        AutomationProperties.SetName(stage, $"Floor {depth} map"); _ = Load();
    }
    private void Navigate(int delta)
    {
        var index = Array.IndexOf(session.Floors, depth) + delta;
        if (index >= 0 && index < session.Floors.Length) ChangeFloor(session.Floors[index]);
    }
    private async Task Expand()
    {
        if (XamlRoot is null) return;
        var view = new LevelMapView(session, depth, true)
        {
            branch = branch, branches = branches, profileKey = profileKey, profileLocationKey = profileLocationKey,
            Width = Math.Clamp(XamlRoot.Size.Width - 120, 320, 1100), Height = Math.Max(300, XamlRoot.Size.Height - 180),
        };
        var dialog = new ContentDialog { XamlRoot = XamlRoot, Title = "Floor maps", Content = view, CloseButtonText = "Close" };
        dialog.Resources["ContentDialogMaxWidth"] = 1200d;
        suspended = true; SyncAnimation();
        try { await dialog.ShowAsync(); }
        finally { suspended = false; requestKey = null; _ = Load(); SyncAnimation(); }
    }
    private double Fit => renderer is null ? 1 : Math.Min(stage.ActualWidth / renderer.Width, stage.ActualHeight / renderer.Height);
    private void Reset() { HideItem(); zoom = 1; panX = panY = 0; Render(); }
    private void Zoom(double value, Point anchor)
    {
        HideItem();
        var next = Math.Clamp(value, 1, 12); var ratio = next / zoom;
        panX = anchor.X - (anchor.X - panX) * ratio; panY = anchor.Y - (anchor.Y - panY) * ratio;
        zoom = next; Constrain(); Render();
    }
    private void Constrain()
    {
        if (renderer is null) return;
        var x = Math.Max(0, (renderer.Width * Fit * zoom - stage.ActualWidth) / 2);
        var y = Math.Max(0, (renderer.Height * Fit * zoom - stage.ActualHeight) / 2);
        panX = Math.Clamp(panX, -x, x); panY = Math.Clamp(panY, -y, y);
    }
    private bool Motion => settings.AnimationsEnabled;
    private void SyncAnimation()
    {
        var active = live && visible && !suspended && XamlRoot?.IsHostVisible == true && renderer?.Animated == true;
        if (active == subscribed) return;
        if (active) CompositionTarget.Rendering += OnRendering; else CompositionTarget.Rendering -= OnRendering;
        subscribed = active;
    }
    private void OnRendering(object? sender, object e)
    {
        var motion = Motion;
        if (motion || lastMotion != motion) Render();
        lastMotion = motion;
    }
    private void Render()
    {
        if (renderer is null || !live || !visible || suspended || rendering || stage.ActualWidth < 1 || stage.ActualHeight < 1) return;
        rendering = true;
        try
        {
            var density = XamlRoot?.RasterizationScale ?? 1;
            var width = Math.Max(1, (int)Math.Round(stage.ActualWidth * density)); var height = Math.Max(1, (int)Math.Round(stage.ActualHeight * density));
            if (bitmap?.PixelWidth != width || bitmap.PixelHeight != height)
            { bitmap = new(width, height); viewport = new uint[width * height]; art.Source = bitmap; }
            var scale = Fit * zoom * density;
            var left = width / 2d + panX * density - renderer.Width * scale / 2;
            var top = height / 2d + panY * density - renderer.Height * scale / 2;
            renderer.RenderViewport(Motion ? clock.Elapsed.TotalMilliseconds : 0, viewport, width, height, scale, left, top);
            using var stream = bitmap.PixelBuffer.AsStream(); stream.Write(MemoryMarshal.AsBytes(viewport.AsSpan())); bitmap.Invalidate();
        }
        finally { rendering = false; }
    }
    private void OnKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Escape && itemCard is not null) { HideItem(); e.Handled = true; return; }

        if (e.OriginalSource is TextBox || Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Control).HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down)
            || Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Menu).HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down)) return;
        switch (e.Key)
        {
            case VirtualKey.I when bundle is not null:
                var tips = bundle.Map.ItemTooltips.Where(tip => session.Secrets || !tip.Hidden).ToArray();
                if (tips.Length > 0) {
                    var index = (Array.FindIndex(tips, tip => tip.Cell == inspectedCell) + 1) % tips.Length;
                    var tip = tips[index]; Reset();
                    InspectItem(new(stage.ActualWidth / 2 + ((tip.Cell % bundle.Map.Width + .5) * 16 - renderer!.Width / 2d) * Fit,
                        stage.ActualHeight / 2 + ((tip.Cell / bundle.Map.Width + .5) * 16 - renderer.Height / 2d) * Fit));
                }
                break;
            case VirtualKey.J when expanded: Navigate(1); break;
            case VirtualKey.K when expanded: Navigate(-1); break;
            case VirtualKey.Add: case (VirtualKey)187: Zoom(zoom * 1.5, new()); break;
            case VirtualKey.Subtract: case (VirtualKey)189: Zoom(zoom / 1.5, new()); break;
            case VirtualKey.Number0: Reset(); break;
            case VirtualKey.Left or VirtualKey.Right or VirtualKey.Up or VirtualKey.Down when floors.FocusState != FocusState.Unfocused || trinkets.FocusState != FocusState.Unfocused: return;
            case VirtualKey.Left: HideItem(); panX += 24; Constrain(); Render(); break;
            case VirtualKey.Right: HideItem(); panX -= 24; Constrain(); Render(); break;
            case VirtualKey.Up: HideItem(); panY += 24; Constrain(); Render(); break;
            case VirtualKey.Down: HideItem(); panY -= 24; Constrain(); Render(); break;
            default: return;
        }
        e.Handled = true;
    }
}
