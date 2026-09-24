using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text.Json;
using Microsoft.UI.Text;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.ApplicationModel.DataTransfer;
using Windows.Graphics;
using Windows.Graphics.Imaging;
using Windows.Storage;
using Windows.System;
using Windows.UI;
using Point = Windows.Foundation.Point;
using Rect = Windows.Foundation.Rect;
using Size = Windows.Foundation.Size;

namespace SeedSeeker;

public sealed partial class MainWindow : Window
{
    private readonly NativeEngine engine = new();
    private readonly ObservableCollection<SeedResult> results = [];
    private QuerySettings query = new() { AutoApplyTrinket = true };
    private List<QueryPreset> userPresets = [];
    private NativeSearch? search;
    /// <summary>The last completed or cancelled traversal; reused only for the same query.</summary>
    private BaseRun? baseRun;
    /// <summary>Every loaded or discovered seed and its source, retained until Clear results.</summary>
    private TargetRun? target;
    /// <summary>Includes both verification and scanning, preventing overlapping actions.</summary>
    private bool busy;
    /// <summary>Every unique seed the current run has delivered, beyond the display cap;
    /// what a concluded run folds into <see cref="target"/> or records in
    /// <see cref="baseRun"/>, so no match is ever lost to the display limit.</summary>
    private readonly List<string> collected = [];
    private readonly HashSet<string> collectedSet = [];
    private readonly Dictionary<string, SeedResult> collectedRecipes = [];
    private bool restoring = true;
    /// <summary>
    /// Anchor for result navigation: the seed of the most recent scout
    /// request, set synchronously so rapid steps chain while a scout is in
    /// flight. A failed request falls back to <see cref="renderedSeed"/>.
    /// </summary>
    private string? scoutedSeed;
    private ScoutItemMappings? itemMappings;
    /// <summary>The seed whose manifest the scout pane currently shows.</summary>
    private string? renderedSeed;
    /// <summary>Only the latest scout request may publish its manifest.</summary>
    private int scoutGeneration;
    private QuerySettings? renderedScoutQuery;
    private ScrollViewer? scoutScroll;
    private TrinketDeckView? scoutTrinkets;
    private (int Depth, double Offset)? scoutAnchor;
    private readonly TranslateTransform trinketDockTransform = new();
    private bool scoutLoading;
    private LevelMapSession? mapSession;
    private Expander? openMap;
    private int? openMapDepth;

    private bool searchRunning;
    /// <summary>
    /// The query that produced the current results, snapshotted at search
    /// start (or import) so an export never reflects later editor changes.
    /// </summary>
    private QuerySettings? searchedQuery;
    /// <summary>
    /// A shared link that arrived before the root element loaded: applying it
    /// may need a ContentDialog, which needs a live XamlRoot.
    /// </summary>
    private string? pendingLink;
    /// <summary>Only the latest copy may reset the checkmark back to the link glyph.</summary>
    private int copyLinkFeedback;
    private const int ResultCap = SearchLimits.ResultCap;
    private static readonly string SettingsPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Seed Seeker", "query.json");
    private static readonly string PresetsPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Seed Seeker", "presets.json");
    private static readonly string UpdateStatePath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Seed Seeker", "update.json");
    /// <summary>
    /// The worker count is a property of this machine, not of the hunt, so it
    /// is saved on its own rather than in query.json — nothing that carries a
    /// query (preset, export, share link) can then pick it up.
    /// </summary>
    private static readonly string WorkersPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Seed Seeker", "workers.json");
    /// <summary>Search threads every start passes to the engine; see <see cref="WorkerPreference"/>.</summary>
    private int workers = WorkerPreference.Ceiling;
    private bool updateCheckStarted;

    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(nint hwnd);

    public MainWindow()
    {
        InitializeComponent();
        // A chip drag: the pressed chip keeps the pointer, so its moves and
        // release bubble up here from wherever it is (see the board section).
        Root.AddHandler(UIElement.PointerMovedEvent, new PointerEventHandler(Root_PointerMoved), true);
        Root.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler(Root_PointerReleased), true);
        Root.AddHandler(UIElement.KeyDownEvent, new KeyEventHandler(Drag_KeyDown), true);
        SystemBackdrop = new MicaBackdrop();
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "SeedSeeker.ico"));
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        AppWindow.Resize(new SizeInt32((int)(1280 * scale), (int)(740 * scale)));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = (int)(1020 * scale);
            presenter.PreferredMinimumHeight = (int)(620 * scale);
        }
        // The upstream version is the engine's own, as the About dialog has it,
        // so the footer cannot fall behind a profile bump.
        UpstreamLink.Content = $"Shattered Pixel Dungeon v{EngineInfo.ShpdVersion}";
        // Decode the item atlases up front so the first sprite render is warm.
        _ = ItemAtlas.GetAsync();
        ResultsList.ItemsSource = results; ScoutButton.IsEnabled = false;
        TrinketDock.RenderTransform = trinketDockTransform;
        ScoutList.Loaded += (_, _) =>
        {
            scoutScroll = Descendants<ScrollViewer>(ScoutList).FirstOrDefault();
            if (scoutScroll is not null) scoutScroll.ViewChanged += (_, _) => UpdateTrinketDock();
        };
        ScoutList.LayoutUpdated += (_, _) =>
        {
            RestoreScoutAnchor();
            UpdateTrinketDock();
        };
        results.CollectionChanged += (_, _) => UpdateResultNav();
        // J/K step the scout pane through the search results from anywhere in
        // the window except a focused text field.
        if (Content is UIElement root) root.KeyDown += Root_KeyDown;
        // The slider indexes into FloorLimits.Options so empty boss floors (5, 10, 15) are not
        // offered; the converter keeps the thumb tooltip showing the floor, not the raw index.
        FloorSlider.ThumbToolTipValueConverter = new FloorLimitIndexConverter();
        FloorSlider.Minimum = 0; FloorSlider.Maximum = FloorLimits.Options.Length - 1; FloorSlider.Value = 0;
        results.CollectionChanged += (_, _) => UpdateTransferButtons();
        LoadWorkerPreference();
        LoadSettings(); LoadPresets(); RefreshPresets(); RefreshQuery(); UpdateTransferButtons();
        Closed += (_, _) => { search?.Cancel(); search?.Dispose(); };
        // ContentDialog needs a live XamlRoot, which only exists once the root
        // element has loaded; Activated can fire before that.
        ((FrameworkElement)Content).Loaded += (_, _) =>
        {
            if (!updateCheckStarted) { updateCheckStarted = true; _ = CheckForUpdatesAsync(); }
            if (pendingLink is string link) { pendingLink = null; _ = ApplySharedLinkAsync(link); }
        };
    }

    private sealed class UpdateState { public string? SkippedVersion { get; set; } public DateTimeOffset LastChecked { get; set; } }

    private sealed record BaseRun(QuerySettings Query, long ResumeFrom, long Remaining);

    private async Task CheckForUpdatesAsync()
    {
        UpdateState state = new();
        try { if (File.Exists(UpdateStatePath)) state = JsonSerializer.Deserialize<UpdateState>(File.ReadAllText(UpdateStatePath)) ?? new(); } catch { }
        var forced = !string.IsNullOrEmpty(UpdateChecker.FakeLatest);
        if (!forced && DateTimeOffset.UtcNow - state.LastChecked < TimeSpan.FromDays(1)) return;
        state.LastChecked = DateTimeOffset.UtcNow;
        SaveUpdateState(state);
        var version = typeof(MainWindow).Assembly.GetName().Version;
        var current = version is null ? "0.0.0" : $"{version.Major}.{version.Minor}.{version.Build}";
        var update = await UpdateChecker.CheckAsync(current);
        if (update is null || update.Version == state.SkippedVersion) return;
        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = "Update available",
            Content = $"Seed Seeker {update.Version} is available on GitHub. You have {current}.",
            PrimaryButtonText = "Download",
            SecondaryButtonText = "Skip this version",
            CloseButtonText = "Not now",
            DefaultButton = ContentDialogButton.Primary,
        };
        var result = await dialog.ShowAsync();
        if (result == ContentDialogResult.Primary)
            Process.Start(new ProcessStartInfo(update.Url) { UseShellExecute = true });
        else if (result == ContentDialogResult.Secondary)
        {
            state.SkippedVersion = update.Version;
            SaveUpdateState(state);
        }
    }

    private static void SaveUpdateState(UpdateState state)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(UpdateStatePath)!);
            File.WriteAllText(UpdateStatePath, JsonSerializer.Serialize(state));
        }
        catch { }
    }

    private void LoadSettings()
    {
        restoring = true;
        try { if (File.Exists(SettingsPath)) query = JsonSerializer.Deserialize<QuerySettings>(File.ReadAllText(SettingsPath)) ?? new(); } catch { query = new(); }
        // Settings saved before empty boss floors were removed may hold 5/10/15; snap them below.
        query.MaximumDepth = FloorLimits.Normalize(query.MaximumDepth);
        foreach (var requirement in query.Requirements)
            if (requirement.MaximumDepth is int depth) requirement.MaximumDepth = FloorLimits.Normalize(depth);
        AutoTrinketToggle.IsOn = query.AutoApplyTrinket;
        FloorSlider.Value = FloorLimits.IndexOf(query.MaximumDepth); RequireBlacksmith.IsOn = query.RequireBlacksmith; ExcludeRewards.IsOn = query.ExcludeBlacksmithRewards;
        WandmakerQuestPicker.ItemsSource = WandmakerQuests.All.Select(WandmakerQuests.Label).ToList();
        WandmakerQuestPicker.SelectedIndex = Array.IndexOf(WandmakerQuests.All, query.WandmakerQuest);
        restoring = false;
    }
    private void SaveSettings() { if (restoring) return; Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!); File.WriteAllText(SettingsPath, JsonSerializer.Serialize(query, new JsonSerializerOptions { WriteIndented = true })); }
    /// <summary>
    /// Restores the device-local worker count and sizes its slider to the
    /// engine's ceiling. A machine with one logical processor has nothing to
    /// choose, so the whole Performance cell goes away there.
    /// </summary>
    private void LoadWorkerPreference()
    {
        var ceiling = WorkerPreference.Ceiling;
        if (ceiling <= 1) { workers = 1; PerformanceCell.Visibility = Visibility.Collapsed; return; }
        restoring = true;
        workers = WorkerPreference.Load(WorkersPath, ceiling);
        WorkerSlider.Maximum = ceiling; WorkerSlider.Value = workers;
        WorkerLabel.Text = WorkerPreference.Describe(workers, ceiling);
        restoring = false;
    }
    private void WorkerSlider_ValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e)
    {
        if (restoring || WorkerLabel is null) return;
        var ceiling = WorkerPreference.Ceiling;
        workers = WorkerPreference.Clamp((int)e.NewValue, ceiling);
        WorkerLabel.Text = WorkerPreference.Describe(workers, ceiling);
        // Local only: this deliberately does not touch SaveSettings, which
        // writes the query.
        WorkerPreference.Save(WorkersPath, workers, ceiling);
    }
    private void LoadPresets()
    {
        try
        {
            if (File.Exists(PresetsPath))
                userPresets = (JsonSerializer.Deserialize<List<QueryPreset>>(File.ReadAllText(PresetsPath)) ?? [])
                    .Where(x => !string.IsNullOrWhiteSpace(x.Name) && x.Query is not null).ToList();
        }
        catch { userPresets = []; }
    }
    private void SavePresets()
    {
        Directory.CreateDirectory(Path.GetDirectoryName(PresetsPath)!);
        File.WriteAllText(PresetsPath, JsonSerializer.Serialize(userPresets, new JsonSerializerOptions { WriteIndented = true }));
    }
    private void RefreshPresets()
    {
        PresetPicker.ItemsSource = BuiltInPresets.All.Concat(userPresets).ToList();
        PresetPicker.SelectedIndex = -1; DeletePresetButton.IsEnabled = false;
    }
    private void ApplyQuery(QuerySettings value)
    {
        restoring = true; query = value.Clone();
        query.MaximumDepth = FloorLimits.Normalize(query.MaximumDepth);
        foreach (var requirement in query.Requirements)
            if (requirement.MaximumDepth is int depth) requirement.MaximumDepth = FloorLimits.Normalize(depth);
        AutoTrinketToggle.IsOn = query.AutoApplyTrinket;
        FloorSlider.Value = FloorLimits.IndexOf(query.MaximumDepth); RequireBlacksmith.IsOn = query.RequireBlacksmith;
        ExcludeRewards.IsOn = query.ExcludeBlacksmithRewards;
        WandmakerQuestPicker.SelectedIndex = Array.IndexOf(WandmakerQuests.All, query.WandmakerQuest);
        restoring = false; RefreshQuery(); SaveSettings();
    }
    private void PresetPicker_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (PresetPicker.SelectedItem is not QueryPreset preset) { DeletePresetButton.IsEnabled = false; return; }
        ApplyQuery(preset.Query); DeletePresetButton.IsEnabled = !preset.IsBuiltIn;
    }
    private async void SavePreset_Click(object sender, RoutedEventArgs e)
    {
        var name = new TextBox { Header = "Preset name", PlaceholderText = "My preset", Width = 360 };
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = "Save Preset", PrimaryButtonText = "Save", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary, Content = name };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary || string.IsNullOrWhiteSpace(name.Text)) return;
        var cleanName = name.Text.Trim(); var existing = userPresets.FindIndex(x => string.Equals(x.Name, cleanName, StringComparison.OrdinalIgnoreCase));
        var preset = new QueryPreset { Name = cleanName, Query = query.Clone() };
        if (existing >= 0) { preset.Id = userPresets[existing].Id; userPresets[existing] = preset; } else userPresets.Add(preset);
        SavePresets(); RefreshPresets();
    }
    private void DeletePreset_Click(object sender, RoutedEventArgs e)
    {
        if (PresetPicker.SelectedItem is not QueryPreset { IsBuiltIn: false } preset) return;
        userPresets.RemoveAll(x => x.Id == preset.Id); SavePresets(); RefreshPresets();
    }
    private void RefreshQuery()
    {
        BuildFarmingFloors();
        BuildBoard(); NoRequirements.Visibility = !query.Requirements.Any(r => !r.Blanket) && !query.NeedsResin && query.FloorRequirements.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        FloorLabel.Text = $"first {query.MaximumDepth} floor{(query.MaximumDepth == 1 ? "" : "s")}"; RequireBlacksmith.IsEnabled = query.MaximumDepth < ScoutQuests.Window(QuestGiver.Blacksmith).Last; StartButton.IsEnabled = search is not null || (!busy && query.HasRequirements); CopyLinkButton.IsEnabled = !searchRunning && query.HasRequirements;
        var count = BitOperations.PopCount((uint)query.Challenges); ChallengeSummary.Text = count == 0 ? "None" : $"{count} enabled";
    }
    private void BuildFarmingFloors()
    {
        FarmingFloorButtons.Children.Clear(); OtherFloorRequirements.Children.Clear();
        foreach (var depth in FloorRequirement.FarmingFloors)
        {
            var button = new Microsoft.UI.Xaml.Controls.Primitives.ToggleButton
            {
                Content = $"Floor {depth}",
                IsChecked = query.FloorRequirements.Any(floor => floor.Depth == depth && floor.IsFarming),
            };
            button.Click += (_, _) =>
            {
                query.ToggleFarmingFloor(depth);
                restoring = true; FloorSlider.Value = FloorLimits.IndexOf(query.MaximumDepth); restoring = false;
                RefreshQuery(); SaveSettings();
            };
            FarmingFloorButtons.Children.Add(button);
        }
        foreach (var floor in query.FloorRequirements.Where(floor => !floor.IsFarming))
        {
            var row = new StackPanel { Spacing = 4 };
            row.Children.Add(new TextBlock { Text = floor.Summary, TextWrapping = TextWrapping.Wrap });
            var remove = new Button { Content = "Remove" };
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(remove, $"Remove floor {floor.Depth} requirement");
            remove.Click += (_, _) => { query.FloorRequirements.RemoveAll(entry => entry.Depth == floor.Depth); RefreshQuery(); SaveSettings(); };
            row.Children.Add(remove); OtherFloorRequirements.Children.Add(row);
        }
    }

    private void FloorSlider_ValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e) { if (restoring || FloorLabel is null) return; query.MaximumDepth = FloorLimits.Options[Math.Clamp((int)e.NewValue, 0, FloorLimits.Options.Length - 1)]; RefreshQuery(); SaveSettings(); }
    /// <summary>
    /// Two setting cards to a row while each can be at least this wide; below
    /// it the Blacksmith toggles and the quest picker have no room left beside
    /// their labels, so the cells fall back into one column.
    /// </summary>
    private const double SettingsPairMinimum = 190;
    private bool settingsPaired = true;
    private void SettingsGrid_SizeChanged(object sender, SizeChangedEventArgs e)
    {
        var paired = (e.NewSize.Width - SettingsGrid.ColumnSpacing) / 2 >= SettingsPairMinimum;
        if (paired == settingsPaired) return;
        settingsPaired = paired;
        FrameworkElement[] cells = [ScopeCell, WandmakerCell, BlacksmithCell, PerformanceCell, RoomsCell];
        for (var i = 0; i < cells.Length; i++)
        {
            Grid.SetRow(cells[i], paired ? i / 2 : i);
            Grid.SetColumn(cells[i], paired ? i % 2 : 0);
            Grid.SetColumnSpan(cells[i], paired ? 1 : 2);
        }
    }
    private void SettingChanged(object sender, RoutedEventArgs e) { if (restoring) return; query.AutoApplyTrinket = AutoTrinketToggle.IsOn; query.RequireBlacksmith = RequireBlacksmith.IsOn; query.ExcludeBlacksmithRewards = ExcludeRewards.IsOn; SaveSettings(); }
    private void WandmakerQuestChanged(object sender, SelectionChangedEventArgs e)
    {
        if (restoring) return;
        var index = WandmakerQuestPicker.SelectedIndex;
        query.WandmakerQuest = index >= 0 && index < WandmakerQuests.All.Length ? WandmakerQuests.All[index] : WandmakerQuest.Any;
        SaveSettings();
    }

    private async void AddRequirement_Click(object sender, RoutedEventArgs e)
    {
        var blanket = sender is FrameworkElement { Tag: true };
        var kind = blanket ? query.Requirements.FirstOrDefault(r => !r.Blanket)?.Kind ?? ItemKind.Weapon : ItemKind.Weapon;
        var requirement = new ItemRequirement { Kind = kind, Blanket = blanket, UpgradeMatch = UpgradeMatch.Any };
        if (await EditRequirement(requirement, StackShape.Lone, blanket ? "New Blanket Requirement" : "New Requirement", "Add") is not { } stack) return;
        SetRequirements(QueryRelationships.ApplyEdit(query.Requirements, null, requirement, stack.Count, stack.Total, stack.CopyDepth));
    }
    /// <summary>Opens the chip at <paramref name="index"/> in the editor; the stack it reports comes back through <see cref="QueryRelationships.ApplyEdit"/>.</summary>
    private async Task EditChip(int index)
    {
        if (index < 0 || index >= query.Requirements.Count) return;
        var shape = QueryRelationships.ItemOf(query.Requirements, index) is { } item ? StackShape.Of(query.Requirements, item) : StackShape.Lone;
        var copy = query.Requirements[index].Clone();
        if (await EditRequirement(copy, shape, copy.Blanket ? "Edit Blanket Requirement" : "Edit Requirement", "Save") is not { } stack) return;
        SetRequirements(QueryRelationships.ApplyEdit(query.Requirements, index, copy, stack.Count, stack.Total, stack.CopyDepth));
    }
    /// <summary>Deletes the chip at <paramref name="index"/>: a whole board entry with its hidden copies, or one member of a cluster.</summary>
    private void RemoveChip(int index)
    {
        if (QueryRelationships.ItemOf(query.Requirements, index) is not { } item) return;
        SetRequirements(item.Cluster is null
            ? QueryRelationships.RemoveItem(query.Requirements, item)
            : QueryRelationships.RemoveMember(query.Requirements, index));
    }
    /// <summary>Adopts an edited requirement list, then redraws and saves.</summary>
    private void SetRequirements(IEnumerable<ItemRequirement> requirements)
    {
        query.Requirements = new ObservableCollection<ItemRequirement>(requirements);
        RefreshQuery(); SaveSettings();
    }

    // ---- the requirement board ----------------------------------------------
    // Every requirement is a chip: drop one chip onto another for an either/or
    // cluster, drag a chip out of its cluster onto the empty board to make it
    // standalone again, drop it on the zone below to remove it. Everything else
    // is a property of the chip itself — a stack badge (×N / ≤N) for "more of
    // the same kind", and a Σ badge for a stack counting its levels together.
    // The board is rebuilt from QueryRelationships.BoardItems on every change.
    //
    // The drag is the board's own, driven by pointer events the way the web
    // board's is, not the system's. A chip is a Button, and ButtonBase takes
    // the pointer for itself on the press, so the framework's CanDrag gesture
    // never starts from one; a press that travels DragThreshold is a drag
    // here instead. The pointer stays captured by the chip, so the moves and
    // the release bubble up to Root, which hosts the handlers; the drop target
    // is whichever chip, capsule, the remove zone or the board lies under the
    // pointer, and a ghost of the chip follows it in DragLayer with the drop's
    // name as its caption, as on the web.

    /// <summary>How far a press travels before it is a drag rather than a click, in DIPs.</summary>
    private const double DragThreshold = 5;
    /// <summary>A chip pressed with the mouse or a pen, from the press until the release.</summary>
    private sealed class ChipPress
    {
        public required long Key; public required Button Chip; public required uint PointerId;
        public required Point Origin; public bool Dragging;
    }
    private ChipPress? press;
    /// <summary>
    /// Raised for the span of a drag: ButtonBase raises Click on the release
    /// whenever the pointer comes back down over the chip it left from, and
    /// that click must not open the editor. Cleared once the release has been
    /// dealt with, whichever order the chip's Click and Root's release run in.
    /// </summary>
    private bool dragClickGuard;
    private enum DropKind { Chip, Cluster, Board, Remove }
    /// <param name="Key">The chip's key, or a cluster's anchor key.</param>
    private sealed record DropTarget(DropKind Kind, FrameworkElement Element, long Key = 0);
    /// <summary>The board's drop targets, every chip before the capsule around it, so the hit test finds the member first.</summary>
    private readonly List<DropTarget> dropTargets = [];
    /// <summary>The ghost chip riding under the pointer, its caption pill, and the target lit beneath it.</summary>
    private Border? ghost; private Border? ghostCaption; private TextBlock? ghostCaptionText;
    private DropTarget? litTarget; private Action? unlight;
    /// <summary>Chip tooltips taken away for the drag, so none opens over the ghost.</summary>
    private readonly List<(Button Chip, object Tip)> suspendedToolTips = [];

    private static Brush ChipFill => ThemeBrush("CardBackgroundFillColorSecondaryBrush", Microsoft.UI.Colors.Transparent);
    private static Brush ChipEdge => ThemeBrush("CardStrokeColorDefaultBrush", Microsoft.UI.Colors.Gray);
    private static Brush DangerInk => ThemeBrush("SystemFillColorCriticalBrush", Microsoft.UI.Colors.IndianRed);
    private static Brush CautionInk => ThemeBrush("SystemFillColorCautionBrush", Microsoft.UI.Colors.Goldenrod);
    private static Brush CautionFill => ThemeBrush("SystemFillColorCautionBackgroundBrush", Microsoft.UI.Colors.Transparent);
    private static Brush SuccessInk => ThemeBrush("SystemFillColorSuccessBrush", Microsoft.UI.Colors.MediumSeaGreen);
    private static Brush SuccessFill => ThemeBrush("SystemFillColorSuccessBackgroundBrush", Microsoft.UI.Colors.Transparent);
    private static FontFamily Mono => new("Cascadia Mono, Consolas");

    /// <summary>The index of the requirement carrying <paramref name="key"/>, or -1.</summary>
    private int IndexOfKey(long key)
    {
        for (var index = 0; index < query.Requirements.Count; index++) if (query.Requirements[index].Key == key) return index;
        return -1;
    }

    /// <summary>Where the chip keyed <paramref name="key"/> stands now: its index and its board entry.</summary>
    private (int Index, BoardItem? Item) Locate(long key)
    {
        var index = IndexOfKey(key);
        return (index, index < 0 ? null : QueryRelationships.ItemOf(query.Requirements, index));
    }

    /// <summary>
    /// Rebuilds the board: a chip per visible requirement, the members of a
    /// cluster inside one capsule, and the dashed "+ Add" chip at the end.
    /// </summary>
    private void BuildBoard()
    {
        CancelDrag();
        RequirementBoard.Children.Clear(); BlanketBoard.Children.Clear(); dropTargets.Clear();
        BlanketHeader.Text = $"Blanket Requirements ({QueryRelationships.BoardCount(query.Requirements.Where(r => r.Blanket))})";
        var requirements = query.Requirements.ToList();
        foreach (var item in QueryRelationships.BoardItems(requirements))
        {
            // The whole entry is validated at once, so a stack's total is
            // weighed against every member that helps reach it.
            var board = requirements[item.Anchor].Blanket ? BlanketBoard : RequirementBoard;
            var problem = requirements[item.Anchor].Blanket ? null : QueryRelationships.Validate(new QuerySettings
            {
                Requirements = new(item.Members.Concat(item.Extras).Select(index => requirements[index])),
            });
            if (item.Cluster is null) board.Children.Add(Chip(requirements, item, item.Anchor, problem));
            else board.Children.Add(Cluster(requirements, item, problem));
        }
        if (query.NeedsResin) RequirementBoard.Children.Add(ArcaneResinChip());
        RequirementBoard.Children.Add(AddChip());
        BlanketBoard.Children.Add(AddChip(blanket: true));
    }

    /// <summary>
    /// One chip: the sprite with its glow, the name, the qualifiers, and — for
    /// a lone chip — its stack badges. The capsule drags, opens the editor when
    /// clicked, and carries the entry's detail as its tooltip.
    /// </summary>
    private Button Chip(IReadOnlyList<ItemRequirement> requirements, BoardItem item, int index, string? problem)
    {
        var requirement = requirements[index];
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(ChipArt(requirement));
        content.Children.Add(ChipName(requirement));
        foreach (var tag in requirement.Tags)
            content.Children.Add(ChipTagPill(tag.Text, tag.Upgrade ? SuccessInk : CautionInk, tag.Upgrade ? SuccessFill : CautionFill));
        if (EffectBadge(requirement) is UIElement effect) content.Children.Add(effect);
        if (requirement.RequireUncursed) content.Children.Add(ChipTagPill("\u2713", SuccessInk, SuccessFill));
        // A cluster's badges belong to its capsule, not to any one member.
        if (item.Cluster is null) foreach (var badge in StackBadges(requirements, item)) content.Children.Add(badge);
        var chip = RequirementChip(content, requirement.Key, ChipMenu(requirements, item, index), problem);
        ToolTipService.SetToolTip(chip, new TextBlock { Text = QueryRelationships.ChipDetail(requirements, index, item, problem), TextWrapping = TextWrapping.Wrap, MaxWidth = 280 });
        dropTargets.Add(new DropTarget(DropKind.Chip, chip, requirement.Key));
        return chip;
    }

    // Item keys are positive; the query-wide resin requirement has no item row.
    private const long ArcaneResinKey = -1;

    private Button RequirementChip(StackPanel content, long key, MenuFlyout menu, string? problem = null)
    {
        var chip = new Button
        {
            Content = content, Tag = key, Height = 30, MinWidth = 0, MinHeight = 0,
            Padding = new Thickness(9, 0, 8, 0), CornerRadius = new CornerRadius(15),
            BorderThickness = new Thickness(1), BorderBrush = problem is null ? ChipEdge : DangerInk,
            Background = ChipFill, VerticalAlignment = VerticalAlignment.Center,
            ContextFlyout = menu,
        };
        chip.Click += Chip_Click; chip.KeyDown += Chip_KeyDown;
        // ButtonBase marks the press handled; the drag listens regardless.
        chip.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler(Chip_PointerPressed), true);
        chip.AddHandler(UIElement.PointerCaptureLostEvent, new PointerEventHandler(Chip_PointerCaptureLost), true);
        chip.AddHandler(UIElement.PointerCanceledEvent, new PointerEventHandler(Chip_PointerCaptureLost), true);
        return chip;
    }

    private StackPanel ArcaneResinContent()
    {
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(new SpriteView { SpriteIndex = 317, SpriteSize = 20, VerticalAlignment = VerticalAlignment.Center });
        content.Children.Add(new TextBlock { Text = "Arcane Resin", FontSize = 13, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center });
        content.Children.Add(ChipTagPill(query.ArcaneResinAuto ? "Auto" : $"≥{query.ArcaneResin}", SuccessInk, SuccessFill));
        if (query.ArcaneResinFilter.MaximumDepth is int depth) content.Children.Add(ChipTagPill($"F≤{depth}", CautionInk, CautionFill));
        if (query.ArcaneResinFilter.IncludeMageWand) content.Children.Add(ChipTagPill("Mage +2", SuccessInk, SuccessFill));
        if (query.ArcaneResinFilter.Uncursed) content.Children.Add(ChipTagPill("\u2713", SuccessInk, SuccessFill));
        return content;
    }

    private Button ArcaneResinChip()
    {
        var menu = new MenuFlyout();
        var edit = new MenuFlyoutItem { Text = "Edit\u2026" };
        edit.Click += async (_, _) => await EditArcaneResin();
        var remove = new MenuFlyoutItem { Text = "Remove", Icon = new FontIcon { Glyph = "" } };
        remove.Click += (_, _) => RemoveArcaneResin();
        menu.Items.Add(edit); menu.Items.Add(new MenuFlyoutSeparator()); menu.Items.Add(remove);
        var chip = RequirementChip(ArcaneResinContent(), ArcaneResinKey, menu);
        var detail = $"{(query.ArcaneResinAuto ? "Auto" : $"≥{query.ArcaneResin}")} Arcane Resin\n{query.ArcaneResinFilter.Summary}";
        ToolTipService.SetToolTip(chip, new TextBlock { Text = detail, TextWrapping = TextWrapping.Wrap, MaxWidth = 280 });
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(chip, detail);
        dropTargets.Add(new DropTarget(DropKind.Chip, chip, ArcaneResinKey));
        return chip;
    }

    private void RemoveArcaneResin()
    {
        query.ArcaneResin = 0; query.ArcaneResinAuto = false; query.ArcaneResinFilter = new();
        SaveSettings(); RefreshQuery();
    }

    /// <summary>
    /// The real item sprite when one is pinned; a grayscale category sprite and
    /// green question mark for wildcards. A requirement names no seed,
    /// so a ring here keeps its class's catalog cell rather than any run's gem.
    /// </summary>
    private static Grid ChipArt(ItemRequirement requirement)
    {
        var art = new Grid { Width = 18, Height = 18, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, -2, 0) };
        if (requirement.Item is null) art.Children.Add(new WildcardSpriteView(requirement.Kind));
        else art.Children.Add(new SpriteView { SpriteIndex = requirement.SpriteIndex, TypeIconIndex = requirement.TypeIconIndex, SpriteSize = 18, GlowColor = requirement.GlowColor, GlowPeriod = requirement.GlowPeriod });
        return art;
    }

    private static TextBlock ChipName(ItemRequirement requirement) =>
        new() { Text = requirement.ShortTitle, FontSize = 13, FontWeight = FontWeights.SemiBold, MaxWidth = 150, TextTrimming = TextTrimming.CharacterEllipsis, VerticalAlignment = VerticalAlignment.Center };

    /// <summary>An either/or cluster: its members share one dashed capsule, with "or" between them and the stack badges at the trailing edge.</summary>
    private Grid Cluster(IReadOnlyList<ItemRequirement> requirements, BoardItem item, string? problem)
    {
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 2, Margin = new Thickness(4, 3, 4, 3), VerticalAlignment = VerticalAlignment.Center };
        for (var position = 0; position < item.Members.Count; position++)
        {
            if (position > 0) row.Children.Add(new TextBlock { Text = "or", FontFamily = Mono, FontSize = 11, FontWeight = FontWeights.Bold, Foreground = CautionInk, Margin = new Thickness(4, 0, 4, 0), VerticalAlignment = VerticalAlignment.Center });
            row.Children.Add(Chip(requirements, item, item.Members[position], problem));
        }
        foreach (var badge in StackBadges(requirements, item)) { badge.Margin = new Thickness(3, 0, 3, 0); row.Children.Add(badge); }
        var capsule = new Grid { Tag = requirements[item.Anchor].Key, VerticalAlignment = VerticalAlignment.Center };
        capsule.Children.Add(DashedCapsule(20, CautionInk, CautionFill));
        capsule.Children.Add(row);
        // After its members, which Chip() has already listed.
        dropTargets.Add(new DropTarget(DropKind.Cluster, capsule, requirements[item.Anchor].Key));
        return capsule;
    }

    /// <summary>The dashed "+ Add" chip that closes the board; Ctrl+N reaches it from anywhere.</summary>
    private Button AddChip(bool blanket = false)
    {
        var label = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 5, VerticalAlignment = VerticalAlignment.Center };
        label.Children.Add(new FontIcon { Glyph = "", FontSize = 12, VerticalAlignment = VerticalAlignment.Center });
        label.Children.Add(new TextBlock { Text = "Add", FontSize = 13, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center });
        label.Margin = new Thickness(12, 0, 12, 0);
        var content = new Grid();
        content.Children.Add(DashedCapsule(15, ChipEdge, ThemeBrush("SubtleFillColorTransparentBrush", Microsoft.UI.Colors.Transparent)));
        content.Children.Add(label);
        var chip = new Button
        {
            Content = content, Height = 30, MinWidth = 0, MinHeight = 0, Padding = new Thickness(0),
            CornerRadius = new CornerRadius(15), BorderThickness = new Thickness(0),
            Background = ThemeBrush("SubtleFillColorTransparentBrush", Microsoft.UI.Colors.Transparent),
            Foreground = ThemeBrush("TextFillColorSecondaryBrush", Microsoft.UI.Colors.Gray),
            VerticalAlignment = VerticalAlignment.Center,
        };
        chip.Tag = blanket;
        ToolTipService.SetToolTip(chip, blanket ? "Add a blanket requirement" : "Add a requirement");
        if (!blanket) chip.KeyboardAccelerators.Add(new KeyboardAccelerator { Modifiers = VirtualKeyModifiers.Control, Key = VirtualKey.N });
        chip.Click += AddRequirement_Click;
        return chip;
    }

    /// <summary>A tiny monospace pill: the chip's tier, upgrade and floor qualifiers.</summary>
    private static Border ChipTagPill(string text, Brush ink, Brush fill) => new()
    {
        Background = fill, CornerRadius = new CornerRadius(4), Padding = new Thickness(4, 0, 4, 0), VerticalAlignment = VerticalAlignment.Center,
        Child = new TextBlock { Text = text, FontFamily = Mono, FontSize = 11, FontWeight = FontWeights.SemiBold, Foreground = ink },
    };

    /// <summary>
    /// What a single pulse cannot say: several effects at once, shown as their
    /// count, and "any enchantment", which settles on no colour. A single
    /// effect — enchantment or curse — needs no badge of its own: the sprite is
    /// already pulsing that very colour, and the tooltip names it.
    /// </summary>
    private static UIElement? EffectBadge(ItemRequirement requirement)
    {
        if (requirement.Effect.AnyEnchantment) return Dot(Rainbow());
        return requirement.Effect.Effects.Count > 1
            ? new EffectCountView(requirement.Effect.Effects)
            : null;
    }

    private static Microsoft.UI.Xaml.Shapes.Ellipse Dot(Brush fill) => new()
    {
        Width = 10, Height = 10, Fill = fill, StrokeThickness = 1, VerticalAlignment = VerticalAlignment.Center,
        Stroke = ThemeBrush("CardStrokeColorDefaultBrush", Microsoft.UI.Colors.Gray),
    };

    /// <summary>Every enchantment colour at once, for the "any enchantment" dot.</summary>
    private static Brush Rainbow()
    {
        var brush = new LinearGradientBrush { StartPoint = new Windows.Foundation.Point(0, 0), EndPoint = new Windows.Foundation.Point(1, 1) };
        var colors = new[] { 0xff5555u, 0xffff55u, 0x55ff55u, 0x55ffffu, 0x5555ffu, 0xff55ffu };
        for (var index = 0; index < colors.Length; index++)
            brush.GradientStops.Add(new GradientStop
            {
                Offset = index / (double)(colors.Length - 1),
                Color = Color.FromArgb(255, (byte)(colors[index] >> 16), (byte)(colors[index] >> 8), (byte)colors[index]),
            });
        return brush;
    }

    /// <summary>The dashed outline of a cluster capsule or the "+ Add" chip; WinUI dashes only shapes.</summary>
    private static Microsoft.UI.Xaml.Shapes.Rectangle DashedCapsule(double radius, Brush stroke, Brush fill)
    {
        var dashes = new DoubleCollection(); dashes.Add(3); dashes.Add(3);
        return new Microsoft.UI.Xaml.Shapes.Rectangle { RadiusX = radius, RadiusY = radius, Stroke = stroke, StrokeThickness = 1, StrokeDashArray = dashes, Fill = fill };
    }

    /// <summary>
    /// The stack badges of one board entry: how many items it asks for and,
    /// when it counts levels, the total they reach. Each opens a flyout that
    /// adjusts it; the edit lands when the flyout closes, so the board is
    /// rebuilt once rather than under the pointer.
    /// </summary>
    private List<Button> StackBadges(IReadOnlyList<ItemRequirement> requirements, BoardItem item)
    {
        var badges = new List<Button>();
        var anchorKey = requirements[item.Anchor].Key;
        // The same rule as the menu's: an entry that cannot copy its kind may
        // only shrink. Only a hand-written document brings it a badge at all.
        var ceiling = QueryRelationships.CanStack(requirements, item) ? SearchLimits.StackMax : item.StackCount;
        if (item.StackCount > 1)
            badges.Add(StackBadge(item.Total is null ? $"\u00d7{item.StackCount}" : $"\u2264{item.StackCount}", SuccessInk, SuccessFill,
                "How many", item.StackCount, 1, ceiling,
                value => { if (Locate(anchorKey).Item is { } entry) SetRequirements(QueryRelationships.SetStackCount(query.Requirements, entry, value)); }));
        if (item.Total is int total)
        {
            // Never a total the stack cannot reach: each ring counts its upgrade
            // plus one, and a world levels only one ring past the standard roll.
            var capacity = QueryRelationships.RingStackCapacity(item.StackCount);
            badges.Add(StackBadge($"\u03a3 \u2265 {total}", CautionInk, CautionFill,
                "Combined level", total, 1, Math.Max(1, capacity),
                value => { if (Locate(anchorKey).Item is { } entry) SetRequirements(QueryRelationships.SetStackTotal(query.Requirements, entry, value)); }));
        }
        return badges;
    }

    private static Button StackBadge(string text, Brush ink, Brush fill, string header, int value, int minimum, int maximum, Action<int> apply)
    {
        var box = new NumberBox { Header = header, Value = value, Minimum = minimum, Maximum = maximum, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline, Width = 180 };
        var flyout = new Flyout { Content = box };
        flyout.Closed += (_, _) => { if (!double.IsNaN(box.Value) && (int)box.Value != value) apply(Math.Clamp((int)box.Value, minimum, maximum)); };
        return new Button
        {
            Content = new TextBlock { Text = text, FontFamily = Mono, FontSize = 11, FontWeight = FontWeights.Bold, Foreground = ink },
            Background = fill, BorderThickness = new Thickness(0), CornerRadius = new CornerRadius(9),
            Padding = new Thickness(6, 0, 6, 0), MinWidth = 0, MinHeight = 0, Height = 18,
            VerticalAlignment = VerticalAlignment.Center, Flyout = flyout,
        };
    }

    /// <summary>The chip's menu: every gesture of the board said in words, for the keyboard and for touch.</summary>
    private MenuFlyout ChipMenu(IReadOnlyList<ItemRequirement> requirements, BoardItem item, int index)
    {
        var key = requirements[index].Key;
        var menu = new MenuFlyout();
        var edit = new MenuFlyoutItem { Text = "Edit\u2026" };
        edit.Click += async (_, _) => await EditChip(IndexOfKey(key));
        menu.Items.Add(edit);
        // "Either/or with…" names the other chips, the menu's way of saying the
        // drop a pointer would make.
        var join = new MenuFlyoutSubItem { Text = "Either/or with\u2026" };
        foreach (var other in QueryRelationships.BoardItems(requirements).Where(entry => !entry.Members.Contains(index) && requirements[entry.Anchor].Blanket == requirements[index].Blanket).SelectMany(entry => entry.Members))
        {
            var targetKey = requirements[other].Key;
            var choice = new MenuFlyoutItem { Text = requirements[other].ShortTitle };
            choice.Click += (_, _) =>
            {
                var source = IndexOfKey(key); var onto = IndexOfKey(targetKey);
                if (source >= 0 && onto >= 0) SetRequirements(QueryRelationships.JoinAlternatives(query.Requirements, source, onto));
            };
            join.Items.Add(choice);
        }
        if (join.Items.Count > 0) menu.Items.Add(join);
        // A cluster spanning two categories names no kind to copy, so it is
        // offered no stack at all.
        if (QueryRelationships.CanStack(requirements, item))
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            var howMany = new MenuFlyoutSubItem { Text = "How many" };
            for (var wanted = 1; wanted <= SearchLimits.StackMax; wanted++)
            {
                var count = wanted;
                var choice = new RadioMenuFlyoutItem { Text = count.ToString(), GroupName = $"stack:{key}", IsChecked = count == item.StackCount };
                choice.Click += (_, _) => { if (Locate(key).Item is { } entry) SetRequirements(QueryRelationships.SetStackCount(query.Requirements, entry, count)); };
                howMany.Items.Add(choice);
            }
            menu.Items.Add(howMany);
        }
        // Only a lone chip naming one ring can count levels: its copies are the
        // same item over again, and a ring's effect scales with its level, so
        // their upgrades add up to something. No other family's do.
        if (item.Cluster is null && requirements[item.Anchor].Item is not null
            && requirements[item.Anchor].Kind.Family() == ItemKind.Ring && item.StackCount > 1)
        {
            var levels = new MenuFlyoutItem { Text = item.Total is null ? "Count levels together" : "Stop counting levels" };
            levels.Click += (_, _) =>
            {
                if (Locate(key).Item is not { } entry) return;
                SetRequirements(QueryRelationships.SetStackTotal(query.Requirements, entry, entry.Total is null ? Math.Max(1, entry.StackCount) : null));
            };
            menu.Items.Add(levels);
        }
        if (item.Cluster is not null)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
            var detach = new MenuFlyoutItem { Text = "On its own" };
            detach.Click += (_, _) => { var at = IndexOfKey(key); if (at >= 0) SetRequirements(QueryRelationships.Detach(query.Requirements, at)); };
            menu.Items.Add(detach);
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        var remove = new MenuFlyoutItem { Text = "Remove", Icon = new FontIcon { Glyph = "" } };
        remove.Click += (_, _) => RemoveChip(IndexOfKey(key));
        menu.Items.Add(remove);
        return menu;
    }

    private async void Chip_Click(object sender, RoutedEventArgs e)
    {
        if (dragClickGuard) return;
        if ((sender as FrameworkElement)?.Tag is not long key) return;
        if (key == ArcaneResinKey) await EditArcaneResin();
        else await EditChip(IndexOfKey(key));
    }

    private void Chip_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key is not (VirtualKey.Delete or VirtualKey.Back)) return;
        if ((sender as FrameworkElement)?.Tag is not long key) return;
        if (key == ArcaneResinKey) { e.Handled = true; RemoveArcaneResin(); return; }
        var index = IndexOfKey(key);
        if (index < 0) return;
        e.Handled = true; RemoveChip(index);
    }

    // ---- the drag ----

    private void Chip_PointerPressed(object sender, PointerRoutedEventArgs e)
    {
        if (sender is not Button chip || chip.Tag is not long key) return;
        // Touch holds for the menu and the right button opens it: only the
        // left mouse button or a pen tip picks a chip up.
        var point = e.GetCurrentPoint(Root);
        if (e.Pointer.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Touch || !point.Properties.IsLeftButtonPressed) return;
        // A press on a stack badge belongs to the badge's flyout.
        for (var node = e.OriginalSource as DependencyObject; node is not null && node != chip; node = VisualTreeHelper.GetParent(node))
            if (node is Button) return;
        CancelDrag();
        dragClickGuard = false;
        press = new ChipPress { Key = key, Chip = chip, PointerId = e.Pointer.PointerId, Origin = point.Position };
    }

    private void Root_PointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if (press is not { } current || e.Pointer.PointerId != current.PointerId) return;
        var position = e.GetCurrentPoint(Root).Position;
        if (!current.Dragging)
        {
            var dx = position.X - current.Origin.X; var dy = position.Y - current.Origin.Y;
            if (dx * dx + dy * dy < DragThreshold * DragThreshold) return;
            BeginDrag(current);
        }
        MoveDrag(position);
    }

    private void Root_PointerReleased(object sender, PointerRoutedEventArgs e)
    {
        ReleaseClickGuard();
        if (press is not { } current || e.Pointer.PointerId != current.PointerId) return;
        FinishPress(e.GetCurrentPoint(Root).Position);
    }

    /// <summary>
    /// ButtonBase lets the pointer go as part of the release, which is the
    /// drop again if Root has not seen it yet; losing the pointer while it is
    /// still down — the window deactivated, the press cancelled — is a cancel.
    /// </summary>
    private void Chip_PointerCaptureLost(object sender, PointerRoutedEventArgs e)
    {
        if (!e.Pointer.IsInContact) ReleaseClickGuard();
        if (press is not { } current || e.Pointer.PointerId != current.PointerId || !ReferenceEquals(sender, current.Chip)) return;
        if (e.Pointer.IsInContact) { CancelDrag(); return; }
        FinishPress(e.GetCurrentPoint(Root).Position);
    }

    private void Drag_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Escape || press is not { Dragging: true }) return;
        e.Handled = true; CancelDrag();
    }

    /// <summary>
    /// Lets the next click through, once the release now being dispatched has
    /// run its course — whichever of the chip's Click and Root's release fires
    /// first, the guard is still up for both. A press cancelled with Escape
    /// keeps its guard until this release, so the click it ends in stays quiet.
    /// </summary>
    private void ReleaseClickGuard() { if (dragClickGuard) DispatcherQueue.TryEnqueue(() => dragClickGuard = false); }

    private void BeginDrag(ChipPress current)
    {
        var index = IndexOfKey(current.Key);
        if (index < 0 && current.Key != ArcaneResinKey) { press = null; return; }
        current.Dragging = true; dragClickGuard = true;
        current.Chip.Opacity = 0.35;
        RemoveZone.Visibility = Visibility.Visible;
        foreach (var target in dropTargets)
            if (target.Element is Button chip && ToolTipService.GetToolTip(chip) is { } tip) { suspendedToolTips.Add((chip, tip)); ToolTipService.SetToolTip(chip, null); }
        ghost = current.Key == ArcaneResinKey ? GhostChip(ArcaneResinContent()) : GhostChip(query.Requirements[index]);
        DragLayer.Children.Add(ghost);
    }

    private void MoveDrag(Point position)
    {
        if (press is not { Dragging: true } current || ghost is null || ghostCaption is null || ghostCaptionText is null) return;
        var target = TargetAt(position);
        var caption = DropCaption(current.Key, target);
        Light(caption is null ? null : target);
        ghostCaption.Visibility = caption is null ? Visibility.Collapsed : Visibility.Visible;
        if (caption is not null && target is not null)
        {
            ghostCaptionText.Text = caption;
            ghostCaption.Background = target.Kind switch
            {
                DropKind.Remove => DangerInk,
                DropKind.Board => ThemeBrush("AccentFillColorDefaultBrush", Microsoft.UI.Colors.DodgerBlue),
                _ => CautionInk,
            };
            ghostCaptionText.Foreground = InkOn(ghostCaption.Background);
        }
        // Under the pointer's left half, as the web ghost rides.
        ghost.Measure(new Size(double.PositiveInfinity, double.PositiveInfinity));
        Canvas.SetLeft(ghost, position.X - ghost.DesiredSize.Width * 0.4);
        Canvas.SetTop(ghost, position.Y - ghost.DesiredSize.Height / 2);
    }

    private void FinishPress(Point position)
    {
        if (press is not { } current) return;
        press = null;
        // A press that never became a drag is the click ButtonBase raises itself.
        if (!current.Dragging) return;
        var target = TargetAt(position);
        EndDrag(current);
        CompleteDrop(current.Key, target);
    }

    private void CancelDrag()
    {
        if (press is not { } current) return;
        press = null;
        if (current.Dragging) EndDrag(current);
    }

    private void EndDrag(ChipPress current)
    {
        Light(null);
        current.Chip.Opacity = 1;
        RemoveZone.Visibility = Visibility.Collapsed;
        if (ghost is not null) { DragLayer.Children.Remove(ghost); ghost = null; ghostCaption = null; ghostCaptionText = null; }
        foreach (var (chip, tip) in suspendedToolTips) ToolTipService.SetToolTip(chip, tip);
        suspendedToolTips.Clear();
    }

    /// <summary>The drop target under <paramref name="position"/> (in Root coordinates): a chip before its capsule, then the remove zone, then the board.</summary>
    private DropTarget? TargetAt(Point position)
    {
        foreach (var target in dropTargets) if (Contains(target.Element, position)) return target;
        if (Contains(RemoveZone, position)) return new DropTarget(DropKind.Remove, RemoveZone);
        if (Contains(BlanketBoard, position)) return new DropTarget(DropKind.Board, BlanketBoard);
        if (Contains(RequirementBoard, position)) return new DropTarget(DropKind.Board, RequirementBoard);
        return null;
    }

    private bool Contains(FrameworkElement element, Point position)
    {
        if (element.Visibility == Visibility.Collapsed || element.ActualWidth == 0) return false;
        return element.TransformToVisual(Root).TransformBounds(new Rect(0, 0, element.ActualWidth, element.ActualHeight)).Contains(position);
    }

    /// <summary>What dropping the chip keyed <paramref name="key"/> on <paramref name="target"/> would do, as the ghost's caption; null when nothing.</summary>
    private string? DropCaption(long key, DropTarget? target)
    {
        if (key == ArcaneResinKey) return query.NeedsResin && target?.Kind == DropKind.Remove ? "remove" : null;
        var source = IndexOfKey(key);
        if (source < 0 || target is null) return null;
        switch (target.Kind)
        {
            case DropKind.Chip or DropKind.Cluster:
                // Joining a cluster the chip is in already changes nothing.
                var onto = IndexOfKey(target.Key);
                if (onto < 0 || onto == source) return null;
                return query.Requirements[source].AlternativeGroup is int group && query.Requirements[onto].AlternativeGroup == group ? null : "or";
            case DropKind.Remove: return "remove";
            default: return query.Requirements[source].AlternativeGroup is null ? null : "on its own";
        }
    }

    private void CompleteDrop(long key, DropTarget? target)
    {
        if (target is null || DropCaption(key, target) is null) return;
        if (key == ArcaneResinKey) { RemoveArcaneResin(); return; }
        var source = IndexOfKey(key);
        switch (target.Kind)
        {
            case DropKind.Chip or DropKind.Cluster: SetRequirements(QueryRelationships.JoinAlternatives(query.Requirements, source, IndexOfKey(target.Key))); break;
            case DropKind.Remove: RemoveChip(source); break;
            default: SetRequirements(QueryRelationships.Detach(query.Requirements, source)); break;
        }
    }

    /// <summary>Lights <paramref name="target"/> as the drop's destination, putting back whatever was lit before; null lights nothing.</summary>
    private void Light(DropTarget? target)
    {
        if (ReferenceEquals(target?.Element, litTarget?.Element)) return;
        unlight?.Invoke(); unlight = null; litTarget = target;
        switch (target)
        {
            case { Kind: DropKind.Chip, Element: Button chip }:
                var (edge, fill) = (chip.BorderBrush, chip.Background);
                chip.BorderBrush = CautionInk; chip.Background = CautionFill;
                unlight = () => { chip.BorderBrush = edge; chip.Background = fill; };
                break;
            case { Kind: DropKind.Cluster, Element: Grid { Children: [Microsoft.UI.Xaml.Shapes.Rectangle outline, ..] } }:
                var dashes = outline.StrokeDashArray;
                outline.StrokeDashArray = null; outline.StrokeThickness = 2;
                unlight = () => { outline.StrokeDashArray = dashes; outline.StrokeThickness = 1; };
                break;
            case { Kind: DropKind.Remove }:
                var ink = InkOn(DangerInk);
                var (zoneFill, iconInk, labelInk) = (RemoveZone.Background, RemoveZoneIcon.Foreground, RemoveZoneLabel.Foreground);
                RemoveZone.Background = DangerInk; RemoveZoneIcon.Foreground = ink; RemoveZoneLabel.Foreground = ink;
                unlight = () => { RemoveZone.Background = zoneFill; RemoveZoneIcon.Foreground = iconInk; RemoveZoneLabel.Foreground = labelInk; };
                break;
            case { Kind: DropKind.Board, Element: WrapPanel board }:
                var boardFill = board.Background;
                board.Background = ThemeBrush("SubtleFillColorSecondaryBrush", Microsoft.UI.Colors.Transparent);
                unlight = () => board.Background = boardFill;
                break;
        }
    }

    /// <summary>
    /// Black or white, whichever reads on <paramref name="fill"/>: the system
    /// caution and critical fills are pastel in the dark theme and deep in the
    /// light one, so no one ink suits a pill painted with them.
    /// </summary>
    private static Brush InkOn(Brush fill)
    {
        var c = (fill as SolidColorBrush)?.Color ?? Microsoft.UI.Colors.Black;
        var luminance = (0.299 * c.R + 0.587 * c.G + 0.114 * c.B) / 255;
        return new SolidColorBrush(luminance > 0.55 ? Microsoft.UI.Colors.Black : Microsoft.UI.Colors.White);
    }

    /// <summary>The chip's likeness that follows the pointer: its sprite and name on a solid ground with a shadow, and a pill for the drop's caption.</summary>
    private Border GhostChip(ItemRequirement requirement)
    {
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(ChipArt(requirement));
        content.Children.Add(ChipName(requirement));
        return GhostChip(content);
    }

    private Border GhostChip(StackPanel content)
    {
        ghostCaptionText = new TextBlock { FontFamily = Mono, FontSize = 11, FontWeight = FontWeights.Bold, VerticalAlignment = VerticalAlignment.Center };
        ghostCaption = new Border { Child = ghostCaptionText, CornerRadius = new CornerRadius(8), Padding = new Thickness(5, 0, 5, 0), Height = 16, Visibility = Visibility.Collapsed, VerticalAlignment = VerticalAlignment.Center };
        content.Children.Add(ghostCaption);
        return new Border
        {
            Child = content, Height = 30, Padding = new Thickness(9, 0, 8, 0), CornerRadius = new CornerRadius(15),
            BorderThickness = new Thickness(1), BorderBrush = ThemeBrush("ControlStrongStrokeColorDefaultBrush", Microsoft.UI.Colors.Gray),
            Background = ThemeBrush("SolidBackgroundFillColorSecondaryBrush", Microsoft.UI.Colors.DimGray),
            IsHitTestVisible = false, Shadow = new ThemeShadow(), Translation = new Vector3(0, 0, 32),
            RenderTransformOrigin = new Point(0.5, 0.5), RenderTransform = new ScaleTransform { ScaleX = 1.04, ScaleY = 1.04 },
        };
    }

    private async Task EditArcaneResin()
    {
        var mode = Combo(new[] { "Amount", "Auto" }, query.ArcaneResinAuto ? 1 : 0);
        mode.Header = "Minimum resin";
        var explanation = new TextBlock { Text = "Upgrade each kept wand to +3. Excluded wands and extra copies reserved for reforging need no resin.", TextWrapping = TextWrapping.Wrap };
        var amount = Number("Minimum resin", query.ArcaneResin > 0 ? query.ArcaneResin : 2, 1, 65535);
        var mageWand = new CheckBox { Content = "Include Mage’s starting wand", IsChecked = query.ArcaneResinFilter.IncludeMageWand };
        var mageHelp = new TextBlock { Text = "Add 2 resin from the Magic Missile wand recovered with Wand Preservation when imbuing another wand. The preserved wand is +0, regardless of the staff’s level.", TextWrapping = TextWrapping.Wrap };
        var uncursed = new CheckBox { Content = "Require uncursed wands", IsChecked = query.ArcaneResinFilter.Uncursed };
        var depth = Combo(new[] { "Search limit" }.Concat(Enumerable.Range(1, SearchLimits.MaxDepth).Select(x => $"Floor {x}")), query.ArcaneResinFilter.MaximumDepth ?? 0);
        var source = Combo(new[] { "Any source" }.Concat(Enum.GetValues<ScoutItemSource>().Select(Labels.Source)), query.ArcaneResinFilter.Source is { } selected ? (int)selected + 1 : 0);
        var content = new StackPanel { Spacing = 16 };
        content.Children.Add(mode); content.Children.Add(explanation);
        content.Children.Add(amount); content.Children.Add(mageWand); content.Children.Add(mageHelp); content.Children.Add(uncursed);
        content.Children.Add(new TextBlock { Text = "Wand floor limit" }); content.Children.Add(depth);
        content.Children.Add(new TextBlock { Text = "Wand source" }); content.Children.Add(source);
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = "Arcane Resin", PrimaryButtonText = query.NeedsResin ? "Save" : "Add", CloseButtonText = "Cancel", SecondaryButtonText = query.NeedsResin ? "Remove" : "", DefaultButton = ContentDialogButton.Primary, Content = VerticalScrollView(content, 440, 460) };
        bool ValidAmount() => mode.SelectedIndex == 1 || (double.IsFinite(amount.Value) && amount.Value == Math.Truncate(amount.Value) && amount.Value is >= 1 and <= 65535);
        void UpdateMode()
        {
            amount.Visibility = mode.SelectedIndex == 0 ? Visibility.Visible : Visibility.Collapsed;
            explanation.Visibility = mode.SelectedIndex == 1 ? Visibility.Visible : Visibility.Collapsed;
            dialog.IsPrimaryButtonEnabled = ValidAmount();
        }
        mode.SelectionChanged += (_, _) => UpdateMode();
        UpdateMode();
        amount.ValueChanged += (_, _) => dialog.IsPrimaryButtonEnabled = ValidAmount();
        dialog.PrimaryButtonClick += (_, args) => { if (!ValidAmount()) args.Cancel = true; };
        var result = await dialog.ShowAsync();
        if (result == ContentDialogResult.None) return;
        query.ArcaneResinAuto = result != ContentDialogResult.Secondary && mode.SelectedIndex == 1;
        query.ArcaneResin = result == ContentDialogResult.Secondary || query.ArcaneResinAuto ? 0 : (int)amount.Value;
        query.ArcaneResinFilter = result == ContentDialogResult.Secondary ? new() : new(
            uncursed.IsChecked == true, depth.SelectedIndex == 0 ? null : depth.SelectedIndex,
            source.SelectedIndex == 0 ? null : (ScoutItemSource)(source.SelectedIndex - 1), mageWand.IsChecked == true);
        SaveSettings(); RefreshQuery();
    }

    /// <param name="r">The requirement edited in place; left as it was when the dialog is cancelled.</param>
    /// <param name="stack">The chip's stack as it stands; a cluster member's belongs to the cluster, so its section stays hidden.</param>
    /// <returns>The stack the editor settled on, or null when the dialog was cancelled.</returns>
    private async Task<StackShape?> EditRequirement(ItemRequirement r, StackShape stack, string title, string accept)
    {
        var kind = Combo(Enum.GetValues<ItemKind>().Select(Labels.Kind), (int)r.Kind);
        var item = new ComboBox { HorizontalAlignment = HorizontalAlignment.Stretch };
        var resin = new Button { Content = "Arcane Resin", Visibility = Visibility.Collapsed };
        // The list the combo is filled from, so saving reads back the very
        // entries it offered — including an imported tier-1 item the fresh-pick
        // list hides.
        var itemChoices = new List<CatalogItem>();
        var tierMatch = Combo(["Any tier", "Exactly", "At least", "At most"], (int)r.TierMatch); var selectedTier = r.Tier is >= SearchLimits.ExactTierMin and <= SearchLimits.ExactTierMax ? r.Tier : SearchLimits.ExactTierMin; var tier = Number("Tier", selectedTier, SearchLimits.ExactTierMin, SearchLimits.ExactTierMax); var tierBound = Combo(Enumerable.Range(SearchLimits.BoundedTierMin, SearchLimits.BoundedTierMax - SearchLimits.BoundedTierMin + 1).Select(value => $"Tier {value}"), Math.Clamp(selectedTier, SearchLimits.BoundedTierMin, SearchLimits.BoundedTierMax) - SearchLimits.BoundedTierMin);
        var maximumUpgrade = Math.Max(2, r.UpgradeCeiling); var selectedMinimumUpgrade = Math.Clamp(r.Upgrade, 1, maximumUpgrade - 1);
        var upgradeMatch = Combo(["Any", "Exactly", "At least"], (int)r.UpgradeMatch); var upgrade = Number("Upgrade level", Math.Clamp(r.Upgrade, 1, maximumUpgrade), 1, maximumUpgrade); var upgradeBound = Combo(Enumerable.Range(1, maximumUpgrade - 1).Select(value => $"+{value} or higher"), selectedMinimumUpgrade - 1);
        // Effect: any / any enchantment / a specific set picked from a per-family
        // checkbox grid (enchantments or glyphs, then curses).
        var effectMode = Combo(["Any", "Any enchantment", "Specific\u2026"], r.Effect.AnyEnchantment ? 1 : r.Effect.IsAny ? 0 : 2);
        var effectBoxes = new List<(string Name, bool Curse, CheckBox Box)>();
        var enchantmentLabel = new TextBlock { Style = (Style)Application.Current.Resources["Caption"] };
        var enchantmentPanel = new WrapPanel { Spacing = 4 };
        var curseSection = new StackPanel { Spacing = 4 };
        var cursePanel = new WrapPanel { Spacing = 4 };
        curseSection.Children.Add(new TextBlock { Text = "Curses", Style = (Style)Application.Current.Resources["Caption"] }); curseSection.Children.Add(cursePanel);
        var effectGrid = new StackPanel { Spacing = 4 }; effectGrid.Children.Add(enchantmentLabel); effectGrid.Children.Add(enchantmentPanel); effectGrid.Children.Add(curseSection);
        var selectTrinket = new CheckBox { Content = "Choose matching trinket at +3", IsChecked = r.SelectTrinket };
        var allowTransmutations = new CheckBox { Content = "Allow transmutations", IsChecked = r.TrinketTransmutations > 0 };
        var transmutations = Number("Maximum transmutations", Math.Clamp(r.TrinketTransmutations, 1, 13), 1, 13);
        transmutations.SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline;
        var transmutationLimit = Row("At most", transmutations);
        var transmutationHelp = new TextBlock { Text = "Includes the initial offers. AutoTrinket can use a helpful starting trinket. Scroll availability and effects after transmuting are not simulated.", TextWrapping = TextWrapping.Wrap };
        var excludeResin = new CheckBox { Content = "Exclude from Auto resin", IsChecked = r.ExcludeResin };
        var resinHelp = new TextBlock { Text = "Keep this wand without budgeting resin to upgrade it. Useful for imbuing: resin upgrades do not transfer to the staff. Extra copies are reserved for reforging and never need Auto resin.", TextWrapping = TextWrapping.Wrap };
        var uncursed = new CheckBox { Content = "Require uncursed", IsChecked = r.RequireUncursed };
        var source = Combo(new[] { "Any source" }.Concat(Enum.GetValues<ScoutItemSource>().Select(Labels.Source)), r.Source is null ? 0 : (int)r.Source + 1);
        // How many items of this kind the chip asks for. The relationships
        // themselves — the either/or clusters and the identity labels behind a
        // stack — belong to the board, which writes them through
        // QueryRelationships; the editor only names the shape.
        var count = Number("Total item count", Math.Clamp(stack.Count, 1, SearchLimits.StackMax), 1, SearchLimits.StackMax);
        count.SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline; count.Visibility = stack.InCluster ? Visibility.Collapsed : Visibility.Visible;
        // A stack's extra copies constrain nothing of their own, but a floor
        // limit is a placement bound rather than an item property, so they may
        // carry one of those.
        var copyDepthToggle = new CheckBox { Content = "Limit the extra copies to a floor", IsChecked = stack.CopyDepth is not null };
        var copyDepth = FloorChoice(stack.CopyDepth ?? 4);
        // A combined level: the stack's items count their levels (upgrade plus
        // one each) towards one total, which any subset of them may reach.
        var totalToggle = new CheckBox { Content = "Count levels together", IsChecked = stack.Total is not null };
        var total = new Slider { Minimum = 1, Maximum = 1, StepFrequency = 1, TickFrequency = 1, Value = stack.Total ?? 1, HorizontalAlignment = HorizontalAlignment.Stretch };
        var depthToggle = ToggleRow("Limit this item to a floor", r.MaximumDepth is not null, out var depthRow); var depth = Number("Within first floors", FloorLimits.Normalize(r.MaximumDepth ?? 4), 1, SearchLimits.MaxDepth);
        // Empty boss floors (5, 10, 15) are useless limits: a single upward spin skips to the
        // next real floor, while typed values snap down (10 means the first 10 floors, ≡ 9).
        depth.ValueChanged += (box, args) =>
        {
            if (double.IsNaN(args.NewValue)) return;
            var requested = (int)args.NewValue;
            var previous = double.IsNaN(args.OldValue) ? requested : (int)args.OldValue;
            var target = FloorLimits.SkipTarget(previous, requested);
            if (target != requested) box.Value = target;
        };
        // Each setting is a two-column row, label leading and control trailing,
        // grouped into cards under the macOS editor's section titles. A row
        // mirrors its control's visibility and a section its rows', so the
        // Sync methods below keep toggling only controls.
        var rowLabels = new Dictionary<FrameworkElement, TextBlock>();
        Grid Row(string label, Control control)
        {
            if (control is NumberBox numberBox) numberBox.Header = null;
            var text = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center, TextWrapping = TextWrapping.Wrap };
            var row = new Grid { ColumnSpacing = 12, Visibility = control.Visibility };
            row.ColumnDefinitions.Add(new ColumnDefinition()); row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            control.MinWidth = 210; Grid.SetColumn(control, 1);
            row.Children.Add(text); row.Children.Add(control);
            rowLabels[control] = text;
            control.RegisterPropertyChangedCallback(UIElement.VisibilityProperty, (_, _) => row.Visibility = control.Visibility);
            return row;
        }
        void Relabel(FrameworkElement control, string label) { if (rowLabels.TryGetValue(control, out var text)) text.Text = label; }
        TextBlock SectionTitle(string title) => new() { Text = title, Style = (Style)Application.Current.Resources["BodyStrongTextBlockStyle"], Margin = new Thickness(1, 0, 0, 8) };
        StackPanel Section(TextBlock? title, params UIElement[] rows)
        {
            var body = new StackPanel { Spacing = 12 }; foreach (var row in rows) body.Children.Add(row);
            var section = new StackPanel();
            if (title is not null) section.Children.Add(title);
            section.Children.Add(new Border { Style = (Style)Application.Current.Resources["SettingsCard"], Child = body });
            void Sync() => section.Visibility = rows.Any(row => row.Visibility == Visibility.Visible) ? Visibility.Visible : Visibility.Collapsed;
            foreach (var row in rows) row.RegisterPropertyChangedCallback(UIElement.VisibilityProperty, (_, _) => Sync());
            Sync();
            return section;
        }
        var effectTitle = SectionTitle("Enchantment");
        var content = new StackPanel { Spacing = 16, Padding = new Thickness(2, 4, 2, 4) };
        foreach (var section in new UIElement[] {
            Section(SectionTitle("Item"), Row("Category", kind), Row("Item", item), resin, Row("Tier", tierMatch), Row("Exact tier", tier), Row("Minimum tier", tierBound)),
            Section(SectionTitle("Upgrade level"), Row("Predicate", upgradeMatch), Row("Upgrade level", upgrade), Row("Minimum upgrade", upgradeBound)),
            Section(effectTitle, Row("Effect", effectMode), effectGrid),
            Section(null, allowTransmutations, transmutationLimit, transmutationHelp, selectTrinket, excludeResin, resinHelp, uncursed, Row("Source", source), depthRow, Row("Within first floors", depth)),
            Section(SectionTitle("Stack"), Row("Total item count", count), copyDepthToggle, copyDepth, totalToggle, total) }) content.Children.Add(section);
        allowTransmutations.Checked += (_, _) => { selectTrinket.IsChecked = false; SyncVisibility(); };
        allowTransmutations.Unchecked += (_, _) => SyncVisibility();
        void NormalizeTier()
        {
            var predicate = (TierMatch)Math.Max(0, tierMatch.SelectedIndex);
            selectedTier = predicate is TierMatch.AtLeast or TierMatch.AtMost ? Math.Clamp(selectedTier, SearchLimits.BoundedTierMin, SearchLimits.BoundedTierMax) : Math.Clamp(selectedTier, SearchLimits.ExactTierMin, SearchLimits.ExactTierMax);
            tier.Value = selectedTier; tierBound.SelectedIndex = Math.Clamp(selectedTier, SearchLimits.BoundedTierMin, SearchLimits.BoundedTierMax) - SearchLimits.BoundedTierMin;
        }
        void SyncVisibility()
        {
            var k = (ItemKind)Math.Max(0, kind.SelectedIndex); var trinket = k == ItemKind.Trinket; var generic = item.SelectedIndex == 0 && k.Family() is ItemKind.Weapon or ItemKind.Armor;
            resin.Visibility = !r.Blanket && k == ItemKind.Wand && accept == "Add" ? Visibility.Visible : Visibility.Collapsed;
            excludeResin.Visibility = resinHelp.Visibility = k == ItemKind.Wand && !r.Blanket ? Visibility.Visible : Visibility.Collapsed;
            allowTransmutations.Visibility = trinket ? Visibility.Visible : Visibility.Collapsed;
            transmutationLimit.Visibility = transmutationHelp.Visibility = trinket && allowTransmutations.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
            selectTrinket.Visibility = !r.Blanket && trinket && allowTransmutations.IsChecked != true ? Visibility.Visible : Visibility.Collapsed;
            var predicate = (TierMatch)Math.Max(0, tierMatch.SelectedIndex); var ranged = predicate is TierMatch.AtLeast or TierMatch.AtMost;
            tierMatch.Visibility = generic ? Visibility.Visible : Visibility.Collapsed;
            tier.Visibility = generic && predicate == TierMatch.Exactly ? Visibility.Visible : Visibility.Collapsed;
            tierBound.Visibility = generic && ranged ? Visibility.Visible : Visibility.Collapsed;
            uncursed.Visibility = source.Visibility = depthRow.Visibility = trinket ? Visibility.Collapsed : Visibility.Visible;
            depth.Visibility = !trinket && depthToggle.IsOn ? Visibility.Visible : Visibility.Collapsed;
            Relabel(item, k.RequiresNamedItem() ? Labels.Kind(k).TrimEnd('s') : "Item");
            Relabel(tierBound, predicate == TierMatch.AtLeast ? "Minimum tier" : "Maximum tier");
            // A stack that counts its levels together has identical any-upgrade
            // members, so the upgrade predicate has nothing left to say.
            var counting = CountingLevels();
            var upgradePredicate = (UpgradeMatch)Math.Max(0, upgradeMatch.SelectedIndex); var ringMinimum = k == ItemKind.Ring && upgradePredicate == UpgradeMatch.AtLeast;
            upgradeMatch.Visibility = counting || k.RequiresNamedItem() ? Visibility.Collapsed : Visibility.Visible;
            upgrade.Visibility = !k.RequiresNamedItem() && !counting && (upgradePredicate == UpgradeMatch.Exactly || ringMinimum) ? Visibility.Visible : Visibility.Collapsed;
            Relabel(upgrade, ringMinimum ? "Minimum upgrade" : "Upgrade level");
            upgradeBound.Visibility = !k.RequiresNamedItem() && !counting && upgradePredicate == UpgradeMatch.AtLeast && !ringMinimum ? Visibility.Visible : Visibility.Collapsed;
        }
        // How many items the stack asks for; a half-typed box reads as one.
        int Counted() => double.IsNaN(count.Value) ? 1 : Math.Clamp((int)count.Value, 1, SearchLimits.StackMax);
        bool CountingLevels() => totalToggle.Visibility == Visibility.Visible && totalToggle.IsChecked == true;
        // The stack section: "how many" is a property of every lone chip, while
        // a floor limit for the extra copies and a combined level are the two
        // shapes a stack of more than one can take — the second only for a
        // concrete ring, whose copies are the same item over again.
        void SyncStack()
        {
            var namedOnly = ((ItemKind)Math.Max(0, kind.SelectedIndex)).RequiresNamedItem();
            count.Visibility = r.Blanket || namedOnly || stack.InCluster ? Visibility.Collapsed : Visibility.Visible;
            var many = !r.Blanket && !namedOnly && !stack.InCluster && Counted() > 1;
            // A combined level is a property of a concrete stack of two or more
            // — and of rings only, whose effects scale with their level.
            var ring = ((ItemKind)Math.Max(0, kind.SelectedIndex)).Family() == ItemKind.Ring;
            totalToggle.Visibility = many && item.SelectedIndex > 0 && ring ? Visibility.Visible : Visibility.Collapsed;
            var counting = CountingLevels();
            total.Visibility = counting ? Visibility.Visible : Visibility.Collapsed;
            copyDepthToggle.Visibility = many && !counting ? Visibility.Visible : Visibility.Collapsed;
            copyDepth.Visibility = copyDepthToggle.Visibility == Visibility.Visible && copyDepthToggle.IsChecked == true ? Visibility.Visible : Visibility.Collapsed;
            // Never a total the stack cannot reach: each ring counts its upgrade
            // plus one, and a world levels only one ring past the standard roll.
            if (counting)
            {
                total.Maximum = Math.Max(1, QueryRelationships.RingStackCapacity(Counted()));
                total.Value = Math.Clamp(double.IsNaN(total.Value) ? 1 : total.Value, 1, total.Maximum);
            }
            total.Header = $"Levels reach \u2265 {(int)total.Value} across up to {Counted()}";
            copyDepth.Header = $"Copies within first {FloorOf(copyDepth)} floor{(FloorOf(copyDepth) == 1 ? "" : "s")}";
            SyncVisibility();
        }
        // Only a tier-4 weapon reaches the top of the weapon range, so naming
        // an item or narrowing the tier can lower the ceiling under the value
        // already picked.
        void NormalizeUpgrade()
        {
            var k = (ItemKind)Math.Max(0, kind.SelectedIndex);
            var chosenIndex = item.SelectedIndex - (k.RequiresNamedItem() ? 0 : 1);
            var chosen = chosenIndex >= 0 && chosenIndex < itemChoices.Count ? itemChoices[chosenIndex] : null;
            maximumUpgrade = Math.Max(2, k.MaximumSearchUpgrade(chosen, (TierMatch)Math.Max(0, tierMatch.SelectedIndex), selectedTier));
            var atLeast = upgradeMatch.SelectedIndex == (int)UpgradeMatch.AtLeast;
            upgrade.Maximum = atLeast ? maximumUpgrade - 1 : maximumUpgrade;
            upgrade.Value = Math.Clamp(double.IsNaN(upgrade.Value) ? 1 : upgrade.Value, 1, upgrade.Maximum);
            selectedMinimumUpgrade = Math.Clamp(selectedMinimumUpgrade, 1, maximumUpgrade - 1);
            upgradeBound.Items.Clear(); foreach (var value in Enumerable.Range(1, maximumUpgrade - 1)) upgradeBound.Items.Add($"+{value} or higher"); upgradeBound.SelectedIndex = selectedMinimumUpgrade - 1;
        }
        // Curses are hidden — and dropped from the selection — while the item
        // must be uncursed; the grid itself shows only for "Specific".
        void SyncEffects()
        {
            effectGrid.Visibility = effectMode.Visibility == Visibility.Visible && effectMode.SelectedIndex == 2 ? Visibility.Visible : Visibility.Collapsed;
            var hideCurses = uncursed.IsChecked == true;
            curseSection.Visibility = hideCurses ? Visibility.Collapsed : Visibility.Visible;
            if (hideCurses) foreach (var (_, curse, box) in effectBoxes) if (curse) box.IsChecked = false;
        }
        void PopulateEffects(IReadOnlyCollection<string> selected)
        {
            var k = (ItemKind)Math.Max(0, kind.SelectedIndex);
            effectBoxes.Clear(); enchantmentPanel.Children.Clear(); cursePanel.Children.Clear();
            foreach (var (names, curse, panel) in new[] { (ItemCatalog.EnchantmentsOf(k), false, enchantmentPanel), (ItemCatalog.CursesOf(k), true, cursePanel) })
                foreach (var name in names)
                {
                    var box = new CheckBox { Content = name, IsChecked = selected.Contains(name), MinWidth = 0, Margin = new Thickness(0, 0, 8, 0) };
                    effectBoxes.Add((name, curse, box)); panel.Children.Add(box);
                }
            enchantmentLabel.Text = k.Family() == ItemKind.Armor ? "Glyphs" : "Enchantments"; effectTitle.Text = k.Family() == ItemKind.Armor ? "Glyph" : "Enchantment";
            effectMode.Visibility = k.Family() is ItemKind.Weapon or ItemKind.Armor ? Visibility.Visible : Visibility.Collapsed;
            SyncEffects();
        }
        void Populate()
        {
            var k = (ItemKind)Math.Max(0, kind.SelectedIndex); var oldId = r.Item?.Id; itemChoices.Clear(); itemChoices.AddRange(ItemCatalog.EditorItems(k, r.Item)); item.Items.Clear(); if (!k.RequiresNamedItem()) item.Items.Add($"Any {Labels.Singular(k)}"); foreach (var value in itemChoices) item.Items.Add(value.Name); item.SelectedIndex = Math.Max(0, itemChoices.FindIndex(x => x.Id == oldId) + (k.RequiresNamedItem() ? 0 : 1));
            PopulateEffects(r.Effect.Effects);
            NormalizeUpgrade(); SyncStack();
        }
        kind.SelectionChanged += (_, _) => { r.Item = null; r.Effect = EffectFilter.Any(); effectMode.SelectedIndex = 0; Populate(); }; item.SelectionChanged += (_, _) => { NormalizeUpgrade(); SyncStack(); }; tier.ValueChanged += (_, _) => { if (!double.IsNaN(tier.Value)) selectedTier = (int)tier.Value; NormalizeUpgrade(); }; tierBound.SelectionChanged += (_, _) => { if (tierBound.SelectedIndex >= 0) selectedTier = tierBound.SelectedIndex + SearchLimits.BoundedTierMin; NormalizeUpgrade(); }; tierMatch.SelectionChanged += (_, _) => { NormalizeTier(); NormalizeUpgrade(); SyncVisibility(); }; upgradeMatch.SelectionChanged += (_, _) => { NormalizeUpgrade(); SyncVisibility(); }; upgradeBound.SelectionChanged += (_, _) => { if (upgradeBound.SelectedIndex >= 0) selectedMinimumUpgrade = upgradeBound.SelectedIndex + 1; }; effectMode.SelectionChanged += (_, _) => SyncEffects(); uncursed.Checked += (_, _) => SyncEffects(); uncursed.Unchecked += (_, _) => SyncEffects(); depthToggle.Toggled += (_, _) => depth.Visibility = depthToggle.IsOn ? Visibility.Visible : Visibility.Collapsed;
        count.ValueChanged += (_, _) => SyncStack(); totalToggle.Checked += (_, _) => SyncStack(); totalToggle.Unchecked += (_, _) => SyncStack(); copyDepthToggle.Checked += (_, _) => SyncStack(); copyDepthToggle.Unchecked += (_, _) => SyncStack();
        total.ValueChanged += (_, _) => total.Header = $"Levels reach \u2265 {(int)total.Value} across up to {Counted()}";
        copyDepth.ValueChanged += (_, _) => copyDepth.Header = $"Copies within first {FloorOf(copyDepth)} floor{(FloorOf(copyDepth) == 1 ? "" : "s")}";
        Populate(); NormalizeTier(); SyncStack();
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = title, PrimaryButtonText = accept, CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Primary, Content = VerticalScrollView(content, 510, 460) };
        var duplicateError = new TextBlock { Text = "This trinket is already required. Each trinket appears only once in the deck.", TextWrapping = TextWrapping.Wrap, Visibility = Visibility.Collapsed };
        content.Children.Add(duplicateError);
        dialog.PrimaryButtonClick += (_, args) =>
        {
            var selected = (ItemKind)kind.SelectedIndex == ItemKind.Trinket
                ? itemChoices.ElementAtOrDefault(Math.Max(0, item.SelectedIndex)) : null;
            var duplicate = !r.Blanket && selected is not null && query.Requirements.Any(other =>
                other.Key != r.Key && !other.Blanket && other.Item?.Id == selected.Id);
            duplicateError.Visibility = duplicate ? Visibility.Visible : Visibility.Collapsed;
            args.Cancel = duplicate;
        };
        var resinRequested = false;
        resin.Click += (_, _) => { resinRequested = true; dialog.Hide(); };
        var result = await dialog.ShowAsync();
        if (resinRequested) { await EditArcaneResin(); return null; }
        if (result != ContentDialogResult.Primary) return null;
        r.Kind = (ItemKind)kind.SelectedIndex; r.Item = r.Kind.RequiresNamedItem() ? itemChoices[Math.Max(0, item.SelectedIndex)] : item.SelectedIndex > 0 ? itemChoices[item.SelectedIndex - 1] : null; r.TierMatch = r.Item is null && r.Kind.Family() is ItemKind.Weapon or ItemKind.Armor ? (TierMatch)tierMatch.SelectedIndex : TierMatch.Any; r.Tier = r.TierMatch == TierMatch.Any ? 0 : selectedTier;
        r.UpgradeMatch = (UpgradeMatch)upgradeMatch.SelectedIndex; r.Upgrade = r.UpgradeMatch switch { UpgradeMatch.Any => 0, UpgradeMatch.Exactly => (int)upgrade.Value, UpgradeMatch.AtLeast when r.Kind == ItemKind.Ring => (int)upgrade.Value, UpgradeMatch.AtLeast => selectedMinimumUpgrade, _ => 0 };
        r.RequireUncursed = uncursed.IsChecked == true;
        r.TrinketTransmutations = r.Kind == ItemKind.Trinket && allowTransmutations.IsChecked == true ? Math.Clamp((int)transmutations.Value, 1, 13) : 0;
        r.SelectTrinket = !r.Blanket && r.Kind == ItemKind.Trinket && r.TrinketTransmutations == 0 && selectTrinket.IsChecked == true;
        r.ExcludeResin = !r.Blanket && r.Kind == ItemKind.Wand && excludeResin.IsChecked == true;
        // One checked effect is a single name, as before effect sets existed; an empty "Specific" means any.
        r.Effect = effectMode.Visibility != Visibility.Visible ? EffectFilter.Any() : effectMode.SelectedIndex switch
        {
            1 => EffectFilter.Enchantment(),
            2 => EffectFilter.OneOf(effectBoxes.Where(entry => entry.Box.IsChecked == true && (!entry.Curse || !r.RequireUncursed)).Select(entry => entry.Name)),
            _ => EffectFilter.Any(),
        };
        r.Source = source.SelectedIndex == 0 ? null : (ScoutItemSource)(source.SelectedIndex - 1);
        r.MaximumDepth = depthToggle.IsOn ? FloorLimits.Normalize(Math.Clamp((int)depth.Value, 1, SearchLimits.MaxDepth)) : null;
        // The identity label and the combined level themselves are the stack's
        // encoding, which ApplyEdit writes from the shape returned here.
        if (r.Kind == ItemKind.Trinket)
        {
            r.Source = null; r.MaximumDepth = null; r.RequireUncursed = false;
            r.UpgradeMatch = UpgradeMatch.Any; r.Upgrade = 0; r.Effect = EffectFilter.Any();
            r.IdentityGroup = null; r.LevelSum = null;
            return new StackShape(1, null, null, stack.InCluster);
        }
        if (r.Kind == ItemKind.Artifact)
        {
            r.UpgradeMatch = UpgradeMatch.Any; r.Upgrade = 0;
            r.IdentityGroup = null; r.LevelSum = null;
            return new StackShape(1, null, null, stack.InCluster);
        }
        var settled = CountingLevels();
        return new StackShape(
            r.Blanket || stack.InCluster ? 1 : Counted(),
            settled ? (int)total.Value : null,
            !stack.InCluster && !settled && Counted() > 1 && copyDepthToggle.IsChecked == true ? FloorOf(copyDepth) : null,
            stack.InCluster);
    }
    /// <summary>A floor picker indexing <see cref="FloorLimits.Options"/>, like the sidebar's own slider.</summary>
    private static Slider FloorChoice(int floor) => new()
    {
        Minimum = 0, Maximum = FloorLimits.Options.Length - 1, StepFrequency = 1, TickFrequency = 1,
        Value = FloorLimits.IndexOf(floor), HorizontalAlignment = HorizontalAlignment.Stretch,
        ThumbToolTipValueConverter = new FloorLimitIndexConverter(),
    };
    /// <summary>The floor a <see cref="FloorChoice"/> slider currently names.</summary>
    private static int FloorOf(Slider slider) =>
        FloorLimits.Options[Math.Clamp((int)Math.Round(slider.Value), 0, FloorLimits.Options.Length - 1)];
    private static ComboBox Combo(IEnumerable<string> values, int selected) { var c = new ComboBox { HorizontalAlignment = HorizontalAlignment.Stretch }; foreach (var v in values) c.Items.Add(v); c.SelectedIndex = selected; return c; }
    private static NumberBox Number(string header, double value, double min, double max) => new() { Header = header, Value = value, Minimum = min, Maximum = max, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Compact };
    private static ToggleSwitch ToggleRow(string label, bool isOn, out Grid row)
    {
        var toggle = new ToggleSwitch { IsOn = isOn, MinWidth = 0, Width = 44, OnContent = "", OffContent = "", Margin = new Thickness(0, -6, 0, -6), VerticalAlignment = VerticalAlignment.Center, HorizontalAlignment = HorizontalAlignment.Right };
        row = new Grid { ColumnSpacing = 12 }; row.ColumnDefinitions.Add(new ColumnDefinition()); row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center }; Grid.SetColumn(toggle, 1); row.Children.Add(text); row.Children.Add(toggle);
        return toggle;
    }

    private async void Challenges_Click(object sender, RoutedEventArgs e)
    {
        var secondary = (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];
        var panel = new StackPanel { Width = 400 }; var toggles = new List<(int, ToggleSwitch)>();
        panel.Children.Add(new TextBlock { Text = "Searches simulate runs with the selected challenges enabled.", TextWrapping = TextWrapping.Wrap, Foreground = secondary, Margin = new Thickness(0, 0, 0, 6) });
        foreach (var entry in Challenges.All)
        {
            var row = new Grid { ColumnSpacing = 12, Padding = new Thickness(0, 8, 0, 8) };
            row.ColumnDefinitions.Add(new ColumnDefinition()); row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var text = new StackPanel { Spacing = 1 };
            text.Children.Add(new TextBlock { Text = entry.Label });
            text.Children.Add(new TextBlock { Text = entry.ChangesLevelGeneration ? "changes level generation" : "no effect on seed content", FontSize = 12, Foreground = secondary });
            var toggle = new ToggleSwitch { IsOn = (query.Challenges & entry.Mask) != 0, MinWidth = 0, Width = 44, OnContent = "", OffContent = "", Margin = new Thickness(0, -6, 0, -6), VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(toggle, 1); row.Children.Add(text); row.Children.Add(toggle); panel.Children.Add(row); toggles.Add((entry.Mask, toggle));
        }
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = "Challenges", PrimaryButtonText = "Done", DefaultButton = ContentDialogButton.Primary, Content = VerticalScrollView(panel, 520) };
        await dialog.ShowAsync(); query.Challenges = toggles.Where(x => x.Item2.IsOn).Aggregate(0, (mask, x) => mask | x.Item1); RefreshQuery(); SaveSettings();
    }

    /// <summary>
    /// Attribution facts for the bundled Shattered Pixel Dungeon artwork, matching
    /// the Android About screen so both platforms state the same thing.
    /// </summary>
    private static readonly (string Label, string Value)[] ArtworkAttribution =
    [
        ("Pixel Dungeon", "© 2012–2015 Oleg Dolya / Watabou"),
        ("Shattered Pixel Dungeon", "© 2014–2026 Evan Debenham"),
        ("Upstream", $"Shattered Pixel Dungeon v{EngineInfo.ShpdVersion}"),
        ("Release JAR SHA-256", EngineInfo.ShpdCommit),
        ("Atlas SHA-256", "4774791518f960a4…7e8e7b5706"),
        ("Icon SHA-256", "38df728d32842d9f…24d7eb9b72"),
    ];

    private static Brush ThemeBrush(string key, Color fallback)
    {
        try { return (Brush)Application.Current.Resources[key]; }
        catch { return new SolidColorBrush(fallback); }
    }

    /// <summary>
    /// The app ships GPL-3.0-or-later artwork from Shattered Pixel Dungeon, so it
    /// has to surface the attribution and a way to read the full license text.
    ///
    /// Every passage below is quoted verbatim from README.md, minus its inline link
    /// markup, and matches the Android About screen line for line. Keep it that way:
    /// the app's prose is the project's own, not a second description of it that can
    /// drift. Section titles are the README's own headings.
    /// </summary>
    private async void About_Click(object sender, RoutedEventArgs e)
    {
        var secondary = ThemeBrush("TextFillColorSecondaryBrush", Microsoft.UI.Colors.Gray);
        var accent = ThemeBrush("AccentTextFillColorPrimaryBrush", Microsoft.UI.Colors.SteelBlue);
        var version = typeof(MainWindow).Assembly.GetName().Version;
        var current = version is null ? "0.0.0" : $"{version.Major}.{version.Minor}.{version.Build}";
        var panel = new StackPanel { Spacing = 12, Width = 460 };

        var heading = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 16 };
        heading.Children.Add(new Image { Width = 68, Height = 68, Source = await BrandMarkAsync(), VerticalAlignment = VerticalAlignment.Center });
        heading.Children.Add(new TextBlock { Text = "Seed Seeker", FontSize = 24, FontWeight = FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center });
        panel.Children.Add(heading);

        // The README's opening line sits under its "# Seed Seeker" heading, so it
        // reads as a lede here rather than as a card that would repeat the title
        // above it.
        panel.Children.Add(new TextBlock
        {
            Text = "An extremely fast seed finder for Shattered Pixel Dungeon, written in Rust — with native apps for Android, Linux, macOS, and Windows.",
            TextWrapping = TextWrapping.Wrap,
            Foreground = secondary,
            Margin = new Thickness(4, 0, 4, 0),
        });

        var acknowledgements = AboutSection(panel, "Acknowledgements");
        acknowledgements.Children.Add(AboutText("Seed Seeker reimplements the generation of Shattered Pixel Dungeon by Evan Debenham, itself based on Pixel Dungeon by Oleg Dolya."));
        acknowledgements.Children.Add(AboutText("Elektrochecker's shpd-seed-finder serves as an oracle for this project's parity tests."));

        var license = AboutSection(panel, "License and identity");
        license.Children.Add(AboutText("This project is GPL-3.0-or-later. It contains a derived generation implementation and an unchanged item sprite atlas from Shattered Pixel Dungeon."));
        var attribution = new StackPanel { Spacing = 8 };
        foreach (var (label, value) in ArtworkAttribution)
        {
            var line = new StackPanel { Spacing = 1 };
            line.Children.Add(new TextBlock { Text = label, FontSize = 11, Foreground = accent });
            line.Children.Add(new TextBlock { Text = value, FontSize = 12, Foreground = secondary, TextWrapping = TextWrapping.Wrap });
            attribution.Children.Add(line);
        }
        license.Children.Add(attribution);
        license.Children.Add(FileReader("LICENSE.txt", "Read full license"));

        panel.Children.Add(new TextBlock
        {
            Text = $"Seed Seeker {current} · Shattered Pixel Dungeon v{EngineInfo.ShpdVersion} profile",
            FontSize = 11,
            Foreground = secondary,
            TextWrapping = TextWrapping.Wrap,
            TextAlignment = TextAlignment.Center,
            HorizontalAlignment = HorizontalAlignment.Center,
            Margin = new Thickness(0, 2, 0, 2),
        });

        var dialog = new ContentDialog
        {
            XamlRoot = Content.XamlRoot,
            Title = "About & licenses",
            CloseButtonText = "Close",
            DefaultButton = ContentDialogButton.Close,
            Content = VerticalScrollView(panel, 520, 500),
        };
        await dialog.ShowAsync();
    }

    /// <summary>
    /// A titled card appended to <paramref name="parent"/>, mirroring the Android
    /// About screen's sections; the returned panel holds the section's body.
    /// </summary>
    private static StackPanel AboutSection(StackPanel parent, string title)
    {
        var body = new StackPanel { Spacing = 10 };
        var content = new StackPanel { Spacing = 8 };
        content.Children.Add(new TextBlock { Text = title, FontWeight = FontWeights.SemiBold });
        content.Children.Add(body);
        parent.Children.Add(new Border
        {
            Style = (Style)Application.Current.Resources["SettingsCard"],
            Padding = new Thickness(16),
            Child = content,
        });
        return body;
    }

    private static TextBlock AboutText(string text) => new() { Text = text, TextWrapping = TextWrapping.Wrap };

    /// <summary>
    /// The app icon, for the About dialog's brand mark. The bundled .ico stores its
    /// frames smallest first, so the largest one is picked explicitly rather than
    /// letting the decoder settle for the 16×16 frame.
    /// </summary>
    private static async Task<ImageSource?> BrandMarkAsync()
    {
        try
        {
            var file = await StorageFile.GetFileFromPathAsync(Path.Combine(AppContext.BaseDirectory, "Assets", "SeedSeeker.ico"));
            using var stream = await file.OpenReadAsync();
            var decoder = await BitmapDecoder.CreateAsync(stream);
            var frame = await decoder.GetFrameAsync(0);
            for (uint index = 1; index < decoder.FrameCount; index++)
            {
                var candidate = await decoder.GetFrameAsync(index);
                if (candidate.PixelWidth > frame.PixelWidth) frame = candidate;
            }
            var data = await frame.GetPixelDataAsync(
                BitmapPixelFormat.Bgra8,
                BitmapAlphaMode.Premultiplied,
                new BitmapTransform(),
                ExifOrientationMode.IgnoreExifOrientation,
                ColorManagementMode.DoNotColorManage);
            var bitmap = new WriteableBitmap((int)frame.PixelWidth, (int)frame.PixelHeight);
            WindowsRuntimeBufferExtensions.CopyTo(data.DetachPixelData(), bitmap.PixelBuffer);
            bitmap.Invalidate();
            return bitmap;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>An expander that reads a bundled text file the first time it opens.</summary>
    private static Expander FileReader(string name, string header)
    {
        var expander = new Expander
        {
            Header = header,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Margin = new Thickness(0, 8, 0, 0),
        };
        expander.Expanding += (_, _) => expander.Content ??= BundledText(name);
        return expander;
    }

    private static TextBlock BundledText(string name)
    {
        string text;
        try { text = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Assets", name)); }
        catch (Exception ex) { text = $"{name} could not be read: {ex.Message}"; }
        return new TextBlock
        {
            Text = text,
            FontFamily = new FontFamily("Cascadia Mono, Consolas"),
            FontSize = 11,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = true,
        };
    }

    private static ScrollView VerticalScrollView(UIElement content, double maxHeight, double? width = null)
    {
        var scrollView = new ScrollView
        {
            Content = content,
            HorizontalScrollMode = ScrollingScrollMode.Disabled,
            HorizontalScrollBarVisibility = ScrollingScrollBarVisibility.Hidden,
            MaxHeight = maxHeight,
        };
        if (width is double value) scrollView.Width = value;
        return scrollView;
    }

    private async void Start_Click(object sender, RoutedEventArgs e)
    {
        if (search is not null) { search.Cancel(); StartButton.IsEnabled = false; return; }
        if (busy) return;
        // The engine only reports a generic rejection over the FFI, so the
        // relationship rules are checked here first, with a message that
        // names the offending group.
        if (QueryRelationships.Validate(query) is string problem) { await ShowTransferMessage(problem); return; }
        if (NativeEngine.ImpossibilityReason(query) is string reason)
        {
            SearchStatus.Text = $"Impossible query. {reason}";
            return;
        }
        await SearchPool();
    }

    private async Task SearchPool()
    {
        busy = true;
        var snapshot = query.Clone();
        var previous = baseRun is { } last && ResultsExport.EncodeQueryDocument(last.Query) == ResultsExport.EncodeQueryDocument(snapshot) ? last : null;
        var pool = target;
        SetStatusBar("Verifying previous results…"); SetStartButton(running: true); StartButton.IsEnabled = false;
        try
        {
            var kept = await Task.Run(() => {
                var matches = new List<SeedResult>();
                if (pool is not null)
                    foreach (var group in pool.Seeds.GroupBy(seed => ResultsExport.EncodeQueryDocument(pool.Sources?.GetValueOrDefault(seed) ?? pool.Query)))
                    {
                        var source = pool.Sources?.GetValueOrDefault(group.First()) ?? pool.Query;
                        matches.AddRange(engine.FilterRecipes(snapshot, source, group.Select(seed => pool.Recipes?.GetValueOrDefault(seed) ?? new SeedResult(seed, 0)).ToArray()));
                    }
                return matches;
            });
            results.Clear(); collected.Clear(); collectedSet.Clear(); collectedRecipes.Clear();
            searchedQuery = snapshot;
            Collect(kept);
            var summary = $"Refined: kept {kept.Count} of {pool?.Seeds.Count ?? 0} previous seeds";
            if (previous is { Remaining: 0 }) { SearchStatus.Text = "Completed"; SetStatusBar(summary); return; }
            SetStatusBar($"{summary} — searching for more…");
            search = await Task.Run(() => previous is null ? engine.Start(snapshot, workers) : engine.StartResumed(snapshot, previous.ResumeFrom, previous.Remaining, workers));
            StartButton.IsEnabled = true;
            await RunSearch(search, snapshot, summary: summary);
            await CaptureBaseRun(snapshot, search!);
        }
        catch (Exception ex) { SearchStatus.Text = $"Search failed: {ex.Message}"; baseRun = null; }
        finally { busy = false; search?.Dispose(); search = null; SetStartButton(running: false); StartButton.IsEnabled = query.HasRequirements; }
    }

    /// <summary>
    /// Records every unique delivered seed; the visible list is capped while
    /// the full set stays available as a later refine's filter input.
    /// </summary>
    private void Collect(IEnumerable<SeedResult> recipes)
    {
        var delivered = recipes.ToArray();
        target = TargetRun.Remember(target, searchedQuery ?? query, delivered);
        foreach (var recipe in delivered)
        {
            if (!collectedSet.Add(recipe.Seed)) continue;
            collected.Add(recipe.Seed);
            collectedRecipes[recipe.Seed] = recipe;
            if (results.Count < ResultCap) results.Add(recipe with { Number = results.Count + 1 });
        }
    }
    private async Task CaptureBaseRun(QuerySettings ranQuery, NativeSearch active)
    {
        var status = await Task.Run(active.Status);
        if (status.State == SearchState.Failed) { baseRun = null; return; }
        var (resumeFrom, remaining) = await Task.Run(active.ResumeHint);
        baseRun = new(ranQuery, resumeFrom, remaining);
    }

    /// <summary>
    /// Writes the window-bottom status bar, the sole home of the transient
    /// refine-progress, refined-summary, and result-cap notices; null clears
    /// the text while the bar itself stays put, so the layout never jumps.
    /// </summary>
    private void SetStatusBar(string? text) => StatusBarText.Text = text ?? "";
    /// <summary>Display-truncation notice for the status bar, joined to a run's
    /// summary when one exists. It reports that the *listing* stopped at
    /// <see cref="ResultCap"/> rows — every further find still reaches
    /// <see cref="collected"/> and the Target Set.</summary>
    private static string WithCapNotice(string? summary) => summary is null
        ? "Result limit reached (1,024 seeds)."
        : $"{summary} · Result limit reached (1,024 seeds).";
    private void SetStartButton(bool running)
    {
        StartIcon.Glyph = running ? "" : "";
        StartLabel.Text = running ? "Cancel Search" : "Start Search";
        PresetPicker.IsEnabled = !running;
        SavePresetButton.IsEnabled = !running;
        CopyLinkButton.IsEnabled = !running && query.HasRequirements;
        DeletePresetButton.IsEnabled = !running
            && PresetPicker.SelectedItem is QueryPreset { IsBuiltIn: false };
        searchRunning = running;
        UpdateTransferButtons();
    }

    private void UpdateTransferButtons()
    {
        ImportResultsButton.IsEnabled = !searchRunning;
        ExportResultsButton.IsEnabled = !searchRunning && results.Count > 0 && searchedQuery is not null;
        // `busy` also covers a refine's filter phase, which owns the results
        // even though no native session exists yet.
        ClearResultsButton.IsEnabled = !searchRunning && !busy
            && (results.Count > 0 || collected.Count > 0 || baseRun is not null || target is not null);
    }

    /// <summary>
    /// Returns the results area to its idle state, dropping the Target — its
    /// query, seed set, and coverage — along with the last run's record, so the
    /// next search anchors a new session from scratch. This is the only way to
    /// end a session: every other path keeps it alive.
    /// </summary>
    private void ClearResults_Click(object sender, RoutedEventArgs e)
    {
        if (busy || search is not null) return;
        results.Clear(); collected.Clear(); collectedSet.Clear(); collectedRecipes.Clear();
        baseRun = null; target = null; searchedQuery = null;
        SearchStatus.Text = "Add requirements, then press Start Search."; SetStatusBar(null);
        UpdateTransferButtons();
    }

    private async void ExportResults_Click(object sender, RoutedEventArgs e)
    {
        // Export the query snapshot captured when the results were produced
        // (at search start or import), never the live editor state.
        // `busy` also covers a refine's filter phase, which runs with no
        // native session but replaces the results when it lands.
        if (busy || search is not null || searchedQuery is null || results.Count == 0) return;
        var exportQuery = searchedQuery.Clone();
        var seeds = results.Select(x => x.Seed).ToList();
        var trinkets = results.Select(x => x.SelectedTrinket).ToList();
        var picker = new Windows.Storage.Pickers.FileSavePicker
        {
            SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary,
            SuggestedFileName = ResultsExport.SuggestedFileName,
        };
        picker.FileTypeChoices.Add("Seed Seeker results", [".json"]);
        // Unpackaged apps must bind pickers to the window handle before use.
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var file = await picker.PickSaveFileAsync();
        if (file is null) return;
        try
        {
            var version = typeof(MainWindow).Assembly.GetName().Version;
            var appVersion = version is null ? "dev" : $"{version.Major}.{version.Minor}.{version.Build}";
            var contents = ResultsExport.Encode(exportQuery, seeds, appVersion, trinkets);
            await FileIO.WriteTextAsync(file, contents);
        }
        catch (Exception ex)
        {
            await ShowTransferMessage($"Export failed: {ex.Message}");
        }
    }

    private async void ImportResults_Click(object sender, RoutedEventArgs e)
    {
        if (busy || search is not null) return;
        var picker = new Windows.Storage.Pickers.FileOpenPicker
        {
            SuggestedStartLocation = Windows.Storage.Pickers.PickerLocationId.DocumentsLibrary,
        };
        picker.FileTypeFilter.Add(".json");
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        var file = await picker.PickSingleFileAsync();
        if (file is null) return;
        try
        {
            var properties = await file.GetBasicPropertiesAsync();
            if (properties.Size > (ulong)EngineInfo.ResultsFileMaxBytes)
            {
                await ShowTransferMessage("This file is too large to be a Seed Seeker results file (2 MiB limit).");
                return;
            }
            var text = await FileIO.ReadTextAsync(file);
            // Parse the untrusted file off the UI thread.
            var imported = await Task.Run(() => ResultsExport.Decode(text));
            // A search or refine may have started while the picker or reads
            // were pending.
            if (busy || search is not null)
            {
                await ShowTransferMessage("Stop the search before importing results.");
                return;
            }
            ApplyQuery(imported.Query);
            var snapshot = imported.Query.Clone();
            searchedQuery = snapshot;
            // Imported results carry no traversal state, so the previous
            // search's record — and the seeds collected as its filter input —
            // no longer describe the listed seeds.
            baseRun = null;
            results.Clear(); collected.Clear(); collectedSet.Clear(); collectedRecipes.Clear(); SetStatusBar(null);
            // The engine already deduplicated and capped the imported seeds.
            Collect(imported.Seeds.Select((seed, index) => new SeedResult(seed, index + 1, imported.Trinkets?.ElementAtOrDefault(index))));
            var dropped = imported.Dropped;
            var status = $"Imported {results.Count} seed{(results.Count == 1 ? "" : "s")} from file.";
            if (dropped > 0)
                status += $"\n{dropped} duplicate or over-limit entr{(dropped == 1 ? "y" : "ies")} dropped.";
            if (imported.FileShpdVersion is string fileVersion && fileVersion != EngineInfo.ShpdVersion)
                status += $"\nMade for Shattered Pixel Dungeon v{fileVersion}; this app targets v{EngineInfo.ShpdVersion}, so seeds may generate differently.";
            SearchStatus.Text = status;
            UpdateTransferButtons();
        }
        catch (ResultsExportException ex)
        {
            await ShowTransferMessage(ex.Message);
        }
        catch (Exception ex)
        {
            await ShowTransferMessage($"Import failed: {ex.Message}");
        }
    }

    /// <summary>Applies the query carried by a seedseeker:// activation, cold start or warm redirect.</summary>
    internal void OpenSharedLink(string link)
    {
        if (Content is FrameworkElement { IsLoaded: false }) { pendingLink = link; return; }
        _ = ApplySharedLinkAsync(link);
    }

    private async Task ApplySharedLinkAsync(string link)
    {
        try
        {
            // Decode the untrusted link text off the UI thread.
            var json = await Task.Run(() => NativeEngine.TryDecodeShareText(link))
                ?? throw new ResultsExportException("This link does not contain a valid Seed Seeker query.");
            var decoded = ResultsExport.DecodeQueryDocument(json);
            if (search is not null)
            {
                await ShowTransferMessage("Stop the search before opening a query link.");
                return;
            }
            ApplyQuery(decoded);
            SearchStatus.Text = "Search loaded from link.";
        }
        catch (ResultsExportException ex)
        {
            await ShowTransferMessage(ex.Message);
        }
    }

    private async void CopyLink_Click(object sender, RoutedEventArgs e)
    {
        if (QueryRelationships.Validate(query) is string problem) { await ShowTransferMessage(problem); return; }
        if (NativeEngine.TryEncodeShareLink(ResultsExport.EncodeQueryDocument(query)) is not string link)
        {
            await ShowTransferMessage("This query could not be encoded into a link.");
            return;
        }
        Copy(link);
        // Brief checkmark feedback, matching the other platforms' link buttons.
        var generation = ++copyLinkFeedback;
        CopyLinkIcon.Glyph = "";
        await Task.Delay(1200);
        if (generation == copyLinkFeedback) CopyLinkIcon.Glyph = "";
    }

    private async Task ShowTransferMessage(string message)
    {
        var dialog = new ContentDialog { XamlRoot = Content.XamlRoot, Title = "Seed Seeker", Content = message, CloseButtonText = "OK" };
        await dialog.ShowAsync();
    }

    /// <summary>
    /// Polls the running session into <see cref="SearchStatus"/>. A resumed
    /// refine passes its summary so the status bar keeps reporting the refine
    /// while the scan runs, and so the result-cap notice can join it.
    /// </summary>
    private async Task RunSearch(NativeSearch active, QuerySettings ranQuery, string? summary = null)
    {
        var goal = collected.Count >= ResultCap ? collected.Count + ResultCap : ResultCap;
        var timer = Stopwatch.StartNew(); long lastScanned = 0; var lastTime = 0d;
        while (true)
        {
            await Task.Delay(150); var batch = await Task.Run(() => active.PollRecipes(128)); Collect(batch);
            var status = await Task.Run(active.Status); var seconds = timer.Elapsed.TotalSeconds; var rate = seconds > lastTime ? (status.Scanned - lastScanned) / (seconds - lastTime) : 0; lastScanned = status.Scanned; lastTime = seconds;
            if (status.State == SearchState.Cancelled && collected.Count >= goal) status = status with { State = SearchState.Completed };
            var probability = status.ProbabilityDescription; var tts = status.ProbabilityUnavailable ? "unavailable" : status.Probability > 0 && rate > 0 ? FormatDuration(1 / status.Probability / rate) : "calculating";
            // A concluded run keeps its counter, except where nothing was
            // scanned: an impossible query is proven before the first seed and
            // "0 seeds searched" would read as a malfunction rather than as
            // the proof it is. A failed run's count is unknown.
            var searched = status.Scanned > 0 ? $" · {status.Scanned:N0} seeds searched" : "";
            SearchStatus.Text = status.IsImpossibleQuery && results.Count == 0
                ? $"Impossible query. {NativeEngine.ImpossibilityReason(ranQuery)}"
                : status.State == SearchState.Running ? $"Seed match probability: {probability} · TTS @ {rate:N0} seeds/s: {tts}\nTime elapsed: {FormatDuration(seconds)} · Seeds searched: {status.Scanned:N0}" : status.State switch { SearchState.Completed => $"Completed{searched}", SearchState.Cancelled => $"Cancelled{searched}", _ => $"Failed (error {status.ErrorCode})" };
            // The engine reports a terminal state only once every queued match
            // has been drained, so breaking here never leaves seeds behind —
            // including a session that stopped itself at its accept cap.
            if (status.State == SearchState.Running && collected.Count >= goal) active.Cancel();
            if (status.State != SearchState.Running)
            {
                var window = await Task.Run(active.ResumeHint);
                if (status.State == SearchState.Completed && status.Scanned > 0 && window.Remaining > 0 && collected.Count < goal)
                {
                    active.Dispose();
                    search = active = await Task.Run(() => engine.StartResumed(ranQuery, window.ResumeFrom, window.Remaining, workers));
                    lastScanned = 0;
                    continue;
                }
                break;
            }
        }
        // Only the concluded run announces the cap: a full display during an
        // accumulating scan is the expected state ("searching for more…" says
        // what is happening), and every further find still reached
        // `collected` and the Target.
        SetStatusBar(results.Count >= ResultCap ? WithCapNotice(summary) : summary);
    }
    private static string FormatDuration(double seconds) => seconds switch { < 1 => "less than a second", < 60 => $"{seconds:N0}s", < 3600 => $"{seconds / 60:N1}m", < 86400 => $"{seconds / 3600:N1}h", _ => $"{seconds / 86400:N1}d" };

    private void ResultsList_SelectionChanged(object sender, SelectionChangedEventArgs e) { if (ResultsList.SelectedItem is SeedResult row) { SeedInput.Text = row.Seed; _ = ScoutSeed(row.Seed); } }
    private void ResultsList_DoubleTapped(object sender, DoubleTappedRoutedEventArgs e) { if (ResultsList.SelectedItem is SeedResult row) Copy(row.Seed); }
    private void CopyResult_Click(object sender, RoutedEventArgs e) { if ((sender as FrameworkElement)?.DataContext is SeedResult row) Copy(row.Seed); }
    private void ScoutResult_Click(object sender, RoutedEventArgs e) { if ((sender as FrameworkElement)?.DataContext is SeedResult row) { SeedInput.Text = row.Seed; _ = ScoutSeed(row.Seed); } }
    private void SeedInput_TextChanged(object sender, TextChangedEventArgs e) { var formatted = SeedCode.Format(SeedInput.Text); if (formatted != SeedInput.Text) { SeedInput.Text = formatted; SeedInput.SelectionStart = formatted.Length; } ScoutButton.IsEnabled = SeedCode.IsCanonical(formatted); }
    private void SeedInput_KeyDown(object sender, KeyRoutedEventArgs e) { if (e.Key == VirtualKey.Enter && SeedCode.IsCanonical(SeedInput.Text)) { _ = ScoutSeed(SeedInput.Text); e.Handled = true; } }
    private async void Scout_Click(object sender, RoutedEventArgs e) => await ScoutSeed(SeedInput.Text);
    private List<string> ResultSeeds() => results.Select(result => result.Seed).ToList();
    /// <summary>Steps the scouted seed through the search results; returns false (inert) when the scouted seed is not one of them or the step cannot move.</summary>
    private bool NavigateResult(int delta)
    {
        if (ResultNavigation.Step(ResultSeeds(), scoutedSeed, delta) is not int target) return false;
        // The selection-changed handler fills the seed field and scouts.
        ResultsList.SelectedIndex = target;
        ResultsList.ScrollIntoView(results[target]);
        return true;
    }
    private void UpdateResultNav()
    {
        if (ResultNavigation.IndexOf(ResultSeeds(), scoutedSeed) is not int index)
        {
            ResultNav.Visibility = scoutTrinkets is null ? Visibility.Collapsed : Visibility.Visible;
            PrevResultButton.Visibility = NextResultButton.Visibility = Visibility.Collapsed;
            ResultPosition.Text = "Trinkets"; ResultNavGuide.Text = "";
            return;
        }
        PrevResultButton.Visibility = NextResultButton.Visibility = Visibility.Visible;
        ResultNavGuide.Text = "J / K";
        ResultNav.Visibility = Visibility.Visible;
        ResultPosition.Text = $"Result {index + 1} of {results.Count}";
        PrevResultButton.IsEnabled = index > 0;
        NextResultButton.IsEnabled = index < results.Count - 1;
    }
    private void PrevResult_Click(object sender, RoutedEventArgs e) => NavigateResult(-1);
    private void NextResult_Click(object sender, RoutedEventArgs e) => NavigateResult(1);
    private void Root_KeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key is not (VirtualKey.J or VirtualKey.K)) return;
        // Never steal letters from a focused text input.
        if (e.OriginalSource is TextBox or NumberBox or AutoSuggestBox or PasswordBox) return;
        if (IsKeyDown(VirtualKey.Control) || IsKeyDown(VirtualKey.Menu)) return;
        // Only swallow the key when navigation actually moved: an inert j/k
        // must stay available to list type-ahead and combo type-select.
        e.Handled = NavigateResult(e.Key == VirtualKey.J ? 1 : -1);
    }
    private static bool IsKeyDown(VirtualKey key) =>
        Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(key)
            .HasFlag(Windows.UI.Core.CoreVirtualKeyStates.Down);

    private async Task ScoutSeed(string seed, string? trinket = null)
    {
        var generation = ++scoutGeneration;
        scoutAnchor = trinket is not null && seed == renderedSeed ? CaptureScoutAnchor() : null;
        scoutLoading = true; SetTrinketDockEnabled(false);
        scoutedSeed = seed; UpdateResultNav();
        ScoutButton.IsEnabled = false; ScoutList.IsEnabled = false; ScoutStatus.Text = "Scouting…";
        try
        {
            // One snapshot names the generated world and its matched item indices.
            var saved = results.FirstOrDefault(result => result.Seed == seed);
            var marked = (trinket is not null && seed == renderedSeed ? renderedScoutQuery
                : saved is not null ? searchedQuery : query)?.Clone() ?? query.Clone();
            var chosen = trinket ?? (saved is not null ? saved.SelectedTrinket ?? "none" : null);
            var world = await Task.Run(() => engine.Scout(seed, marked.Challenges, marked, chosen));
            if (generation != scoutGeneration) return;
            var matches = await Task.Run(() => NativeEngine.ScoutMatches(seed, marked.Challenges, marked, chosen));
            if (generation != scoutGeneration) return;
            var preserveMaps = trinket is not null && seed == renderedSeed && mapSession is not null;
            if (preserveMaps) mapSession!.Update(world, marked);
            else { openMapDepth = null; mapSession = new(world, marked, selected => ScoutSeed(seed, selected)); }
            openMap = null;
            var entries = world.Items.Select((item, index) => (Item: item, Index: index)).ToLookup(x => x.Item.Depth);
            var depths = entries.Select(g => g.Key).Concat(mapSession!.Floors).Distinct().Order().ToArray();
            var matchedChoices = ScoutChoices.Matched(world.Items, matches.Matched);
            var groups = depths.Select(depth =>
            {
                var g = entries[depth];
                var group = new ScoutGroup { Depth = depth, Floor = $"Floor {depth}", Region = Region(depth), Feeling = world.FloorFeelings?.FirstOrDefault(f => f.Depth == depth)?.Feeling ?? FloorFeeling.None,
                    Header = FloorHeader(world, depth) };
                var trinkets = g.Where(entry => entry.Item.Item.Kind == ItemKind.Trinket).ToList();
                foreach (var entry in g)
                {
                    if (entry.Item.Item.Kind != ItemKind.Trinket)
                        group.Add(ScoutRow.From(entry.Item, matches.Matched.Contains(entry.Index), world.Gems,
                            ScoutChoices.Dimmed(entry.Item, matches.Matched.Contains(entry.Index), matchedChoices)));
                    else if (entry.Index == trinkets[0].Index)
                        group.Add(ScoutRow.Catalyst(entry.Item, world.TrinketOrder ?? trinkets.Select(x => x.Item.Item).ToList(),
                            trinkets.Where(x => matches.Matched.Contains(x.Index)).Select(x => x.Item.Item.Id)
                                .Concat((world.TrinketOrder ?? []).Skip(4).Where((_, index) => matches.TransmutedTrinkets.Contains(index)).Select(item => item.Id)).ToHashSet(),
                            world.SelectedTrinket, selected => { _ = ScoutSeed(seed, selected); }));
                }
                if (group.Count == 0) group.Add(new ScoutRow { Depth = depth, ItemName = "No notable items on this floor.", RowOpacity = .65 });
                return group;
            }).ToList();
            scoutTrinkets = groups.SelectMany(group => group).Select(row => row.TrinketDeck).OfType<TrinketDeckView>().FirstOrDefault();
            ScoutList.ItemsSource = new CollectionViewSource { IsSourceGrouped = true, Source = groups }.View;
            BuildTrinketDock(world);
            renderedScoutQuery = marked;
            UpdateResultNav();
            // Slot counts: an "any of these" group is one requirement however many members it has.
            ScoutStatus.Text = $"{world.Items.Count} items across {groups.Count} floors" + (matches.TotalRequirements == 0 ? "" : $"  ·  {matches.MatchedRequirements} of {matches.TotalRequirements} requirement{(matches.TotalRequirements == 1 ? "" : "s")} matched");
            EmptyScout.Visibility = Visibility.Collapsed; ScoutList.Visibility = Visibility.Visible;
            renderedSeed = seed;
            itemMappings = world.ItemMappings;
            SeedInfoButton.Visibility = itemMappings is null ? Visibility.Collapsed : Visibility.Visible;
        }
        catch (Exception ex)
        {
            if (generation != scoutGeneration) return;
            ScoutStatus.Text = ex.Message;
            // Keep the indicator describing the manifest that is still shown.
            scoutAnchor = null; scoutedSeed = renderedSeed; UpdateResultNav();
        }
        finally { if (generation == scoutGeneration) { ScoutButton.IsEnabled = SeedCode.IsCanonical(SeedInput.Text); ScoutList.IsEnabled = true; scoutLoading = false; SetTrinketDockEnabled(true); } }
    }

    private UIElement FloorHeader(ScoutWorld world, int depth)
    {
        var title = new WrapPanel { Spacing = 8, LineSpacing = 4 };
        title.Children.Add(new TextBlock { Text = $"Floor {depth}", FontWeight = FontWeights.SemiBold });
        title.Children.Add(new FloorFeelingView { Feeling = world.FloorFeelings?.FirstOrDefault(f => f.Depth == depth)?.Feeling ?? FloorFeeling.None, VerticalAlignment = VerticalAlignment.Center });
        title.Children.Add(new TextBlock { Text = Region(depth), FontSize = 12, Opacity = .7, VerticalAlignment = VerticalAlignment.Center });
        var quest = QuestLabel(world.Quests, depth);
        if (quest.Length > 0) title.Children.Add(new TextBlock { Text = $"· {quest}", FontSize = 12, Opacity = .7, VerticalAlignment = VerticalAlignment.Center });
        if (world.IsFarmingFloor(depth))
        {
            var garden = new TextBlock { Text = "· Garden", FontSize = 12, Foreground = SuccessInk, VerticalAlignment = VerticalAlignment.Center };
            ToolTipService.SetToolTip(garden, "Dark floor with a garden.");
            title.Children.Add(garden);
        }
        return EngineInfo.MapDepths.Contains(depth) ? MapDisclosure(depth, title) : title;
    }

    private Expander MapDisclosure(int depth, UIElement title)
    {
        var profile = mapSession!;
        var disclosure = new Expander { Header = title, HorizontalAlignment = HorizontalAlignment.Stretch,
            HorizontalContentAlignment = HorizontalAlignment.Stretch, Margin = new Thickness(0, 6, 0, 2) };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(disclosure, $"Floor {depth} map");
        disclosure.Expanding += (_, _) =>
        {
            if (openMap is not null && openMap != disclosure) openMap.IsExpanded = false;
            openMap = disclosure; openMapDepth = depth;
            disclosure.Content = new LevelMapView(profile, depth);
        };
        disclosure.Collapsed += (_, _) =>
        {
            disclosure.Content = null;
            if (openMap == disclosure) { openMap = null; openMapDepth = null; }
        };
        disclosure.KeyDown += (_, e) =>
        {
            if (e.Key != VirtualKey.Escape || !disclosure.IsExpanded) return;
            disclosure.IsExpanded = false; disclosure.Focus(FocusState.Keyboard); e.Handled = true;
        };
        if (openMapDepth == depth) disclosure.IsExpanded = true;
        return disclosure;
    }
    private static IEnumerable<T> Descendants<T>(DependencyObject root) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(root); i++)
        {
            var child = VisualTreeHelper.GetChild(root, i);
            if (child is T match) yield return match;
            foreach (var descendant in Descendants<T>(child)) yield return descendant;
        }
    }

    private List<(int Depth, double Top, double Bottom)> ScoutFloorPositions()
    {
        if (scoutScroll is null) return [];
        return Descendants<ListViewItem>(ScoutList)
            .Where(row => row.Content is ScoutRow && row.ActualHeight > 0)
            .Select(row => (Depth: ((ScoutRow)row.Content).Depth,
                Top: row.TransformToVisual(scoutScroll).TransformPoint(new Point()).Y,
                Height: row.ActualHeight))
            .GroupBy(row => row.Depth).OrderBy(group => group.Key)
            .Select(group => (group.Key, group.Min(row => row.Top), group.Max(row => row.Top + row.Height))).ToList();
    }

    private (int Depth, double Offset)? CaptureScoutAnchor()
    {
        foreach (var floor in ScoutFloorPositions())
            if (floor.Bottom > 0) return (floor.Depth, floor.Top);
        return null;
    }

    private void RestoreScoutAnchor()
    {
        if (scoutLoading || scoutScroll is null || scoutAnchor is not { } anchor) return;
        foreach (var floor in ScoutFloorPositions())
        {
            if (floor.Depth != anchor.Depth) continue;
            scoutAnchor = null;
            scoutScroll.ChangeView(null, Math.Max(0, scoutScroll.VerticalOffset + floor.Top - anchor.Offset), null, true);
            break;
        }
    }

    private void SetTrinketDockEnabled(bool enabled)
    {
        foreach (var button in TrinketDock.Children.OfType<Microsoft.UI.Xaml.Controls.Primitives.ToggleButton>())
            button.IsEnabled = enabled;
        UpdateTrinketDock();
    }

    private void BuildTrinketDock(ScoutWorld world)
    {
        TrinketDock.Children.Clear();
        foreach (var item in (world.TrinketOrder ?? []).Take(4))
        {
            var applied = world.SelectedTrinket == item.Id;
            var button = new Microsoft.UI.Xaml.Controls.Primitives.ToggleButton
            {
                Width = 32, Height = 32, Padding = new Thickness(4), CornerRadius = new CornerRadius(6),
                IsChecked = applied, BorderThickness = new Thickness(applied ? 2 : 1),
                BorderBrush = ThemeBrush(applied ? "SystemFillColorSuccessBrush" : "CardStrokeColorDefaultBrush", applied ? Microsoft.UI.Colors.ForestGreen : Microsoft.UI.Colors.Gray),
                Background = ThemeBrush(applied ? "SystemFillColorSuccessBackgroundBrush" : "CardBackgroundFillColorDefaultBrush", Microsoft.UI.Colors.Transparent),
                Content = new SpriteView { SpriteIndex = item.SpriteIndex, SpriteSize = 20 },
                IsTabStop = false,
            };
            TrinketDeckView.StyleSelection(button);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, item.Name);
            ToolTipService.SetToolTip(button, item.Name);
            button.Click += (_, _) =>
            {
                button.IsChecked = applied;
                if (!scoutLoading) _ = ScoutSeed(world.Seed, applied ? "none" : item.Id);
            };
            TrinketDock.Children.Add(button);
        }
        UpdateTrinketDock();
    }

    private void UpdateTrinketDock()
    {
        var progress = 0d;
        if (scoutScroll is not null && scoutTrinkets?.Choices is { ActualHeight: > 0 } choices
            && choices.XamlRoot is not null)
        {
            var top = choices.TransformToVisual(scoutScroll).TransformPoint(new Point()).Y;
            progress = Math.Clamp(-top / choices.ActualHeight, 0, 1);
        }
        ResultNavGuide.Opacity = 0.6 * (1 - progress);
        TrinketDock.Opacity = progress;
        trinketDockTransform.Y = (1 - progress) * 32;
        TrinketDock.IsHitTestVisible = progress > 0 && !scoutLoading;
        foreach (var button in TrinketDock.Children.OfType<Microsoft.UI.Xaml.Controls.Primitives.ToggleButton>())
            button.IsTabStop = progress > 0 && !scoutLoading;
    }

    private static string Region(int depth) => depth switch { <= 5 => "Sewers", <= 10 => "Prison", <= 15 => "Caves", <= 20 => "Dwarven City", _ => "Demon Halls" };
    /// <summary>The variant label of the quest hosted on <paramref name="depth"/>, or "" for quest-less floors.</summary>
    private static string QuestLabel(IReadOnlyList<ScoutQuest> quests, int depth) =>
        quests.FirstOrDefault(quest => quest.Depth == depth) is { } quest ? ScoutQuests.VariantLabel(quest.Variant) : "";
    private async void SeedInfo_Click(object sender, RoutedEventArgs e)
    {
        if (renderedSeed is string seed && itemMappings is { } mappings)
            await SeedInfoDialog.ShowAsync(((FrameworkElement)Content).XamlRoot, seed, mappings);
    }

    private void CopySeed_Click(object sender, RoutedEventArgs e) { if (SeedCode.IsCanonical(SeedInput.Text)) Copy(SeedInput.Text); }
    private static void Copy(string text) { var data = new DataPackage(); data.SetText(text); Clipboard.SetContent(data); }
}

public sealed class ScoutGroup : List<ScoutRow>
{
    public UIElement? Header { get; init; }
    public int Depth { get; init; }
    public string Floor { get; init; } = "";
    public FloorFeeling Feeling { get; init; }
    public string Region { get; init; } = "";
}

public sealed class ScoutRow
{
    public int Depth { get; init; }
    public UIElement? TrinketDeck { get; init; }
    public static ScoutRow Catalyst(ScoutItem catalyst, IReadOnlyList<CatalogItem> order, IReadOnlySet<string> matches, string? selectedTrinket, Action<string> onSelect) => new()
    {
        Depth = catalyst.Depth, ItemName = "Magical Catalyst", SpriteIndex = 70, Source = Labels.Source(catalyst.Source),
        SecretVisibility = catalyst.Secret ? Visibility.Visible : Visibility.Collapsed,
        Accessibility = catalyst.AccessibilityTag switch { 1 => $"One reward of choice group {ScoutChoices.Letter(catalyst.AccessibilityGroup)}", 2 => $"Only in some outcomes of scenario group {ScoutChoices.Letter(catalyst.AccessibilityGroup)}", _ => "" },
        AccessibilityVisibility = catalyst.AccessibilityTag == 2 ? Visibility.Visible : Visibility.Collapsed,
        Choice = ScoutChoices.Letter(catalyst.AccessibilityGroup), ChoiceVisibility = catalyst.AccessibilityTag == 1 ? Visibility.Visible : Visibility.Collapsed,
        TrinketDeck = new TrinketDeckView(order, matches, selectedTrinket, onSelect),
    };
    public string ItemName { get; init; } = "";
    public string Upgrade { get; init; } = "";
    public Visibility UpgradeVisibility { get; init; } = Visibility.Collapsed;
    public Visibility CurseVisibility { get; init; } = Visibility.Collapsed;
    public Visibility SecretVisibility { get; init; } = Visibility.Collapsed;
    public string Effect { get; init; } = "";
    public Visibility EffectVisibility { get; init; } = Visibility.Collapsed;
    public Brush EffectBrush { get; init; } = new SolidColorBrush(Color.FromArgb(255, 42, 160, 176));
    public string Source { get; init; } = "";
    public string Accessibility { get; init; } = "";
    public Visibility AccessibilityVisibility { get; init; } = Visibility.Collapsed;
    public Visibility MatchVisibility { get; init; } = Visibility.Collapsed;
    public string Choice { get; init; } = "";
    public Visibility ChoiceVisibility { get; init; } = Visibility.Collapsed;
    public double RowOpacity { get; init; } = 1;
    /// <summary>
    /// Row-major index into the upstream item atlas: the cell this run draws the
    /// item in, which for a ring is the gem the seed gave its class.
    /// </summary>
    public int SpriteIndex { get; init; } = -1;
    /// <summary>The ring class's glyph in <c>item_icons.png</c>, or -1 for anything else.</summary>
    public int TypeIconIndex { get; init; } = -1;
    /// <summary>Enchantment/curse glow colour; only meaningful when <see cref="GlowPeriod"/> is positive.</summary>
    public Color GlowColor { get; init; }
    /// <summary>Seconds to peak glow, or zero when the item neither is enchanted nor cursed.</summary>
    public double GlowPeriod { get; init; }
    public Windows.UI.Text.FontWeight Weight { get; init; } = FontWeights.Normal;

    /// <param name="gems">The scouted run's ring gems, which decide the cell a
    /// ring is drawn in; the same item is a different colour in another run.</param>
    public static ScoutRow From(ScoutItem x, bool match, RingGems gems, bool dimmed = false)
    {
        var access = x.AccessibilityTag switch { 1 => $"One reward of choice group {ScoutChoices.Letter(x.AccessibilityGroup)} (option {x.AccessibilityValue + 1})", 2 => $"Only in some outcomes of scenario group {ScoutChoices.Letter(x.AccessibilityGroup)}", _ => "" };
        var isCurse = x.Effect is not null && ItemCatalog.IsCurse(x.Item.Kind, x.Effect);
        var glow = ItemGlow.ForItem(x);
        return new()
        {
            Depth = x.Depth,
            ItemName = x.Item.Name,
            Upgrade = $"+{x.DisplayedUpgrade}", UpgradeVisibility = x.DisplayedUpgrade > 0 ? Visibility.Visible : Visibility.Collapsed,
            CurseVisibility = x.Cursed ? Visibility.Visible : Visibility.Collapsed,
            SecretVisibility = x.Secret ? Visibility.Visible : Visibility.Collapsed,
            Effect = x.Effect ?? "", EffectVisibility = x.Effect is null ? Visibility.Collapsed : Visibility.Visible,
            EffectBrush = isCurse ? (Brush)Application.Current.Resources["SystemFillColorCriticalBrush"] : new SolidColorBrush(Color.FromArgb(255, 42, 160, 176)),
            Source = Labels.Source(x.Source),
            Accessibility = access, AccessibilityVisibility = x.AccessibilityTag == 2 ? Visibility.Visible : Visibility.Collapsed,
            Choice = ScoutChoices.Letter(x.AccessibilityGroup), ChoiceVisibility = x.AccessibilityTag == 1 ? Visibility.Visible : Visibility.Collapsed,
            RowOpacity = dimmed ? .45 : 1,
            MatchVisibility = match ? Visibility.Visible : Visibility.Collapsed,
            Weight = match ? FontWeights.SemiBold : FontWeights.Normal,
            SpriteIndex = gems.SpriteIndex(x.Item), TypeIconIndex = x.Item.TypeIconIndex ?? -1,
            GlowColor = glow?.Color ?? default, GlowPeriod = glow?.Period ?? 0,
        };
    }
}
