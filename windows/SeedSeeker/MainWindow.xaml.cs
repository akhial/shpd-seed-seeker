using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Numerics;
using System.Text.Json;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics;
using Windows.System;

namespace SeedSeeker;

public sealed partial class MainWindow : Window
{
    private readonly NativeEngine engine = new();
    private readonly ObservableCollection<SeedResult> results = [];
    private readonly ObservableCollection<ScoutRow> scoutRows = [];
    private QuerySettings query = new();
    private NativeSearch? search;
    private bool restoring;
    private const int ResultCap = 1024;
    private static readonly string SettingsPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Seed Seeker", "query.json");

    public MainWindow()
    {
        InitializeComponent(); FloorSlider.Value = 1; FloorSlider.Minimum = 1; SystemBackdrop = new MicaBackdrop(); ResultsList.ItemsSource = results; ScoutList.ItemsSource = scoutRows;
        AppWindow.Resize(new SizeInt32(1180, 720)); LoadSettings(); RefreshQuery();
        Closed += (_, _) => { search?.Cancel(); search?.Dispose(); };
    }

    private void LoadSettings()
    {
        restoring = true;
        try { if (File.Exists(SettingsPath)) query = JsonSerializer.Deserialize<QuerySettings>(File.ReadAllText(SettingsPath)) ?? new(); } catch { query = new(); }
        FloorSlider.Value = query.MaximumDepth; RequireBlacksmith.IsChecked = query.RequireBlacksmith; ExcludeRewards.IsChecked = query.ExcludeBlacksmithRewards; FastMode.IsChecked = query.FastMode; restoring = false;
    }
    private void SaveSettings() { if (restoring) return; Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!); File.WriteAllText(SettingsPath, JsonSerializer.Serialize(query, new JsonSerializerOptions { WriteIndented = true })); }
    private void RefreshQuery()
    {
        RequirementList.ItemsSource = query.Requirements; NoRequirements.Visibility = query.Requirements.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        FloorLabel.Text = $"first {query.MaximumDepth} floor{(query.MaximumDepth == 1 ? "" : "s")}"; RequireBlacksmith.IsEnabled = query.MaximumDepth <= 14; StartButton.IsEnabled = query.Requirements.Count != 0;
        var count = BitOperations.PopCount((uint)query.Challenges); ChallengeSummary.Text = $"⚑  Challenges: {count} enabled"; ChallengeSummary.Visibility = count == 0 ? Visibility.Collapsed : Visibility.Visible;
    }
    private void FloorSlider_ValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e) { if (restoring || FloorLabel is null) return; query.MaximumDepth = (int)e.NewValue; RefreshQuery(); SaveSettings(); }
    private void SettingChanged(object sender, RoutedEventArgs e) { query.RequireBlacksmith = RequireBlacksmith.IsChecked == true; query.ExcludeBlacksmithRewards = ExcludeRewards.IsChecked == true; query.FastMode = FastMode.IsChecked == true; SaveSettings(); }

    private async void AddRequirement_Click(object sender, RoutedEventArgs e) { var r = new ItemRequirement { Kind = ItemKind.Weapon, UpgradeMatch = UpgradeMatch.Any }; if (await EditRequirement(r, true)) { query.Requirements.Add(r); RefreshQuery(); SaveSettings(); } }
    private async void Requirement_Tapped(object sender, TappedRoutedEventArgs e)
    {
        if ((sender as FrameworkElement)?.DataContext is not ItemRequirement original) return; var copy = original.Clone();
        if (await EditRequirement(copy, false)) { var index = query.Requirements.IndexOf(original); query.Requirements[index] = copy; RefreshQuery(); SaveSettings(); }
    }
    private void RemoveRequirement_Click(object sender, RoutedEventArgs e) { if ((sender as Button)?.Tag is ItemRequirement r) { query.Requirements.Remove(r); RefreshQuery(); SaveSettings(); } }
    private void RemoveRequirement_Tapped(object sender, TappedRoutedEventArgs e) => e.Handled = true;

    private async Task<bool> EditRequirement(ItemRequirement r, bool isNew)
    {
        var kind = Combo(Enum.GetValues<ItemKind>().Select(Labels.Kind), (int)r.Kind); kind.Header = "Category";
        var item = new ComboBox { Header = "Item", HorizontalAlignment = HorizontalAlignment.Stretch };
        var tierMatch = Combo(["Any tier", "Exactly", "At least"], (int)r.TierMatch); tierMatch.Header = "Tier predicate"; var tier = Number("Tier", r.Tier is >= 2 and <= 5 ? r.Tier : 2, 2, 5);
        var upgradeMatch = Combo(["Any", "Exactly", "At least"], (int)r.UpgradeMatch); upgradeMatch.Header = "Upgrade predicate"; var upgrade = Number("Upgrade level", r.Upgrade, 0, 4);
        var modifier = new ComboBox { Header = "Enchantment or glyph", HorizontalAlignment = HorizontalAlignment.Stretch };
        var source = Combo(new[] { "Any source" }.Concat(Enum.GetValues<ScoutItemSource>().Select(Labels.Source)), r.Source is null ? 0 : (int)r.Source + 1); source.Header = "Source";
        var group = Combo(["None", "A", "B", "C", "D"], r.IdentityGroup ?? 0); group.Header = "Same-item group";
        var depthToggle = new CheckBox { Content = "Limit this item to a floor", IsChecked = r.MaximumDepth is not null }; var depth = Number("Within first floors", r.MaximumDepth ?? 5, 1, 24);
        var content = new StackPanel { Spacing = 12, Padding = new Thickness(2, 4, 2, 4) }; foreach (var control in new UIElement[] { kind, item, tierMatch, tier, upgradeMatch, upgrade, modifier, source, group, depthToggle, depth }) content.Children.Add(control);
        void Populate()
        {
            var k = (ItemKind)Math.Max(0, kind.SelectedIndex); var oldId = r.Item?.Id; var items = ItemCatalog.For(k).ToList(); item.Items.Clear(); item.Items.Add($"Any {Labels.Singular(k)}"); foreach (var value in items) item.Items.Add(value); item.DisplayMemberPath = "Name"; item.SelectedIndex = Math.Max(0, items.FindIndex(x => x.Id == oldId) + 1);
            modifier.Items.Clear(); modifier.Items.Add("None"); foreach (var value in ItemCatalog.Modifiers(k)) modifier.Items.Add(value); modifier.SelectedIndex = r.Modifier is null ? 0 : Math.Max(0, ItemCatalog.Modifiers(k).ToList().IndexOf(r.Modifier) + 1); modifier.Visibility = k is ItemKind.Weapon or ItemKind.Armor ? Visibility.Visible : Visibility.Collapsed;
            tierMatch.Visibility = tier.Visibility = item.SelectedIndex == 0 && k is ItemKind.Weapon or ItemKind.Armor ? Visibility.Visible : Visibility.Collapsed; upgrade.Maximum = k == ItemKind.Ring ? 4 : 3;
        }
        kind.SelectionChanged += (_, _) => { r.Item = null; r.Modifier = null; Populate(); }; item.SelectionChanged += (_, _) => { var k = (ItemKind)Math.Max(0, kind.SelectedIndex); tierMatch.Visibility = tier.Visibility = item.SelectedIndex == 0 && k is ItemKind.Weapon or ItemKind.Armor ? Visibility.Visible : Visibility.Collapsed; }; depthToggle.Click += (_, _) => depth.Visibility = depthToggle.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        Populate(); depth.Visibility = depthToggle.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = isNew ? "New Requirement" : "Edit Requirement", PrimaryButtonText = isNew ? "Add" : "Save", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary, Content = new ScrollViewer { Content = content, MaxHeight = 510, Width = 430 } };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary) return false;
        r.Kind = (ItemKind)kind.SelectedIndex; r.Item = item.SelectedIndex > 0 ? ItemCatalog.For(r.Kind).ElementAt(item.SelectedIndex - 1) : null; r.TierMatch = r.Item is null && r.Kind is ItemKind.Weapon or ItemKind.Armor ? (TierMatch)tierMatch.SelectedIndex : TierMatch.Any; r.Tier = r.TierMatch == TierMatch.Any ? 0 : (int)tier.Value;
        r.UpgradeMatch = (UpgradeMatch)upgradeMatch.SelectedIndex; r.Upgrade = r.UpgradeMatch == UpgradeMatch.Any ? 0 : Math.Max(r.UpgradeMatch == UpgradeMatch.Exactly ? 1 : 0, (int)upgrade.Value); r.Modifier = modifier.Visibility == Visibility.Visible && modifier.SelectedIndex > 0 ? modifier.SelectedItem?.ToString() : null;
        r.Source = source.SelectedIndex == 0 ? null : (ScoutItemSource)(source.SelectedIndex - 1); r.IdentityGroup = group.SelectedIndex == 0 ? null : group.SelectedIndex; r.MaximumDepth = depthToggle.IsChecked == true ? (int)depth.Value : null; return true;
    }
    private static ComboBox Combo(IEnumerable<string> values, int selected) { var c = new ComboBox { HorizontalAlignment = HorizontalAlignment.Stretch }; foreach (var v in values) c.Items.Add(v); c.SelectedIndex = selected; return c; }
    private static NumberBox Number(string header, double value, double min, double max) => new() { Header = header, Value = value, Minimum = min, Maximum = max, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact };

    private async void Challenges_Click(object sender, RoutedEventArgs e)
    {
        var entries = new (int Mask, string Name, bool Changes)[] { (1,"On diet",false),(2,"Faith is my armor",false),(4,"Pharmacophobia",false),(8,"Barren land",true),(16,"Swarm intelligence",false),(32,"Into darkness",true),(64,"Forbidden runes",true),(128,"Hostile champions",false),(256,"Badder bosses",false) };
        var panel = new StackPanel { Spacing = 4, Width = 420 }; var boxes = new List<(int, CheckBox)>(); panel.Children.Add(new TextBlock { Text = "Searches simulate runs with the selected challenges enabled.", TextWrapping = TextWrapping.Wrap });
        foreach (var entry in entries) { var box = new CheckBox { Content = entry.Name, IsChecked = (query.Challenges & entry.Mask) != 0, Margin = new Thickness(0, 8, 0, 0) }; panel.Children.Add(box); panel.Children.Add(new TextBlock { Text = entry.Changes ? "changes level generation" : "no effect on seed content", FontSize = 12, Opacity = .65, Margin = new Thickness(32, -5, 0, 0) }); boxes.Add((entry.Mask, box)); }
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = "Challenges", PrimaryButtonText = "Done", DefaultButton = ContentDialogButton.Primary, Content = panel }; await dialog.ShowAsync(); query.Challenges = boxes.Where(x => x.Item2.IsChecked == true).Aggregate(0, (mask, x) => mask | x.Item1); RefreshQuery(); SaveSettings();
    }

    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        if (search is not null) { search.Cancel(); StartButton.IsEnabled = false; return; } results.Clear(); SearchStatus.Text = "Starting search…"; StartButton.Content = "  Cancel Search";
        try { search = await Task.Run(() => engine.Start(query)); await RunSearch(search); } catch (Exception ex) { SearchStatus.Text = $"Failed: {ex.Message}"; }
        finally { search?.Dispose(); search = null; StartButton.Content = "  Start Search"; StartButton.IsEnabled = query.Requirements.Count != 0; }
    }
    private async Task RunSearch(NativeSearch active)
    {
        var timer = Stopwatch.StartNew(); long lastScanned = 0; var lastTime = 0d;
        while (true)
        {
            await Task.Delay(150); var pollCount = Math.Max(1, Math.Min(128, ResultCap - results.Count)); var batch = await Task.Run(() => active.Poll(pollCount)); foreach (var seed in batch) if (results.Count < ResultCap) results.Add(new(seed, results.Count + 1));
            var status = await Task.Run(active.Status); var seconds = timer.Elapsed.TotalSeconds; var rate = seconds > lastTime ? (status.Scanned - lastScanned) / (seconds - lastTime) : 0; lastScanned = status.Scanned; lastTime = seconds;
            var probability = status.Probability > 0 ? $"{status.Probability:P4}" : "calculating"; var tts = status.Probability > 0 && rate > 0 ? FormatDuration(1 / status.Probability / rate) : "calculating";
            SearchStatus.Text = status.State == SearchState.Running ? $"Seed match probability: {probability}   •   TTS @ {rate:N0} seeds/s: {tts}\nTime elapsed: {FormatDuration(seconds)}" : status.State switch { SearchState.Completed => "Completed", SearchState.Cancelled => "Cancelled", _ => $"Failed (error {status.ErrorCode})" };
            if (results.Count >= ResultCap) { active.Cancel(); SearchStatus.Text += "\nResult limit reached (1,024 seeds)."; } if (status.State != SearchState.Running || results.Count >= ResultCap) break;
        }
    }
    private static string FormatDuration(double seconds) => seconds switch { < 1 => "less than a second", < 60 => $"{seconds:N0}s", < 3600 => $"{seconds / 60:N1}m", < 86400 => $"{seconds / 3600:N1}h", _ => $"{seconds / 86400:N1}d" };

    private void ResultsList_SelectionChanged(object sender, SelectionChangedEventArgs e) { if (ResultsList.SelectedItem is SeedResult row) { SeedInput.Text = row.Seed; _ = ScoutSeed(row.Seed); } }
    private void ResultsList_DoubleTapped(object sender, DoubleTappedRoutedEventArgs e) { if (ResultsList.SelectedItem is SeedResult row) Copy(row.Seed); }
    private void SeedInput_TextChanged(object sender, TextChangedEventArgs e) { var formatted = SeedCode.Format(SeedInput.Text); if (formatted != SeedInput.Text) { SeedInput.Text = formatted; SeedInput.SelectionStart = formatted.Length; } ScoutButton.IsEnabled = SeedCode.IsCanonical(formatted); }
    private void SeedInput_KeyDown(object sender, KeyRoutedEventArgs e) { if (e.Key == VirtualKey.Enter && SeedCode.IsCanonical(SeedInput.Text)) { _ = ScoutSeed(SeedInput.Text); e.Handled = true; } }
    private async void Scout_Click(object sender, RoutedEventArgs e) => await ScoutSeed(SeedInput.Text);
    private async Task ScoutSeed(string seed)
    {
        ScoutButton.IsEnabled = false; ScoutStatus.Text = "Scouting…";
        try { var world = await Task.Run(() => engine.Scout(seed, query.Challenges)); scoutRows.Clear(); foreach (var group in world.Items.GroupBy(x => x.Depth).OrderBy(x => x.Key)) { var first = true; foreach (var item in group) { scoutRows.Add(ScoutRow.From(item, Matches(item), first)); first = false; } } var floors = world.Items.Select(x => x.Depth).Distinct().Count(); var matches = world.Items.Count(Matches); ScoutStatus.Text = $"{world.Items.Count} items across {floors} floors" + (query.Requirements.Count == 0 ? "" : $"  ·  {matches} requirement match{(matches == 1 ? "" : "es")}"); EmptyScout.Visibility = Visibility.Collapsed; ScoutList.Visibility = Visibility.Visible; }
        catch (Exception ex) { ScoutStatus.Text = ex.Message; } finally { ScoutButton.IsEnabled = SeedCode.IsCanonical(SeedInput.Text); }
    }
    private bool Matches(ScoutItem item) => query.Requirements.Any(r => r.Kind == item.Item.Kind && (r.Item is null || r.Item.Id == item.Item.Id) && (r.UpgradeMatch == UpgradeMatch.Any || r.UpgradeMatch == UpgradeMatch.Exactly && item.Upgrade == r.Upgrade || r.UpgradeMatch == UpgradeMatch.AtLeast && item.Upgrade >= r.Upgrade) && (r.Modifier is null || r.Modifier == item.Effect) && (r.Source is null || r.Source == item.Source));
    private void CopySeed_Click(object sender, RoutedEventArgs e) { if (SeedCode.IsCanonical(SeedInput.Text)) Copy(SeedInput.Text); }
    private static void Copy(string text) { var data = new DataPackage(); data.SetText(text); Clipboard.SetContent(data); }
}

public sealed class ScoutRow
{
    public string ItemName { get; init; } = ""; public string Upgrade { get; init; } = ""; public string Curse { get; init; } = ""; public string Detail { get; init; } = ""; public string Accessibility { get; init; } = ""; public string Match { get; init; } = ""; public string Glyph { get; init; } = ""; public Brush Tint { get; init; } = new SolidColorBrush(Colors.Gray); public Windows.UI.Text.FontWeight Weight { get; init; } = FontWeights.Normal;
    public static ScoutRow From(ScoutItem x, bool match, bool first)
    {
        var region = x.Depth switch { <= 5 => "Sewers", <= 10 => "Prison", <= 15 => "Caves", <= 20 => "Dwarven City", _ => "Demon Halls" }; var access = x.AccessibilityTag switch { 1 => $"One reward of choice group {x.AccessibilityGroup} (option {x.AccessibilityValue + 1})", 2 => $"Only in some outcomes of scenario group {x.AccessibilityGroup}", _ => "" };
        return new() { ItemName = x.Item.Name, Upgrade = x.Upgrade > 0 ? $"+{x.Upgrade}" : "", Curse = x.Cursed ? "cursed" : "", Detail = (first ? $"Floor {x.Depth} · {region}  —  " : "") + (x.Effect is null ? "" : x.Effect + " · ") + Labels.Source(x.Source), Accessibility = access, Match = match ? "✓ Match" : "", Weight = match ? FontWeights.SemiBold : FontWeights.Normal, Glyph = x.Item.Kind switch { ItemKind.Weapon => "", ItemKind.Armor => "", ItemKind.Wand => "", _ => "" }, Tint = new SolidColorBrush(x.Item.Kind switch { ItemKind.Weapon => Colors.DarkOrange, ItemKind.Armor => Colors.DodgerBlue, ItemKind.Wand => Colors.MediumPurple, _ => Colors.Goldenrod }) };
    }
}
