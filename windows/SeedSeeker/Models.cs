using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace SeedSeeker;

// This file must stay free of Windows App SDK types: SeedSeeker.Tests links it
// to run on any host. Members that need XAML types live in the partial halves
// in Models.Presentation.cs.

// MeleeWeapon and ThrownWeapon narrow a weapon requirement to one weapon
// class; the enum value indexes the document kind-name table in
// ResultsExport, so they must stay appended after the original four families.
public enum ItemKind { Weapon, Armor, Wand, Ring, MeleeWeapon, ThrownWeapon, Trinket, Artifact }

/// <summary>Melee/thrown classification of weapon catalog entries.</summary>
public enum WeaponClass { Melee, Thrown }

public static class ItemKindExtensions
{
    /// <summary>The broad item family; catalog items always carry the family.</summary>
    public static ItemKind Family(this ItemKind kind) =>
        kind is ItemKind.MeleeWeapon or ItemKind.ThrownWeapon ? ItemKind.Weapon : kind;
}

/// <summary>
/// Local copies of the engine's floor and session limits the window keeps to
/// (<c>crates/seedfinder-core/src/engine_info.rs</c>); EngineConstantsTests
/// asserts each of them against the engine's <c>engine_info</c> document. The
/// requirement editor's own bounds are the shared editor's, which its forms
/// carry.
/// </summary>
public static class SearchLimits
{
    /// <summary>Deepest floor a search may cover.</summary>
    public const int MaxDepth = 24;
    /// <summary>How many results one run lists, and one import restores.</summary>
    public const int ResultCap = 1024;
}

/// <summary>
/// The nine challenges in engine mask order, with the stable document name the
/// results codec writes and whether the level generator consults the
/// challenge (so enabling it changes which seeds match). A local copy of the
/// engine's list, checked against <c>engine_info</c> by EngineConstantsTests.
/// </summary>
public static class Challenges
{
    public sealed record Entry(string Name, int Mask, string Label, bool ChangesLevelGeneration);

    public static readonly Entry[] All =
    [
        new("on_diet", 1, "On diet", false),
        new("faith_is_my_armor", 2, "Faith is my armor", false),
        new("pharmacophobia", 4, "Pharmacophobia", false),
        new("barren_land", 8, "Barren land", true),
        new("swarm_intelligence", 16, "Swarm intelligence", false),
        new("into_darkness", 32, "Into darkness", true),
        new("forbidden_runes", 64, "Forbidden runes", true),
        new("hostile_champions", 128, "Hostile champions", false),
        new("badder_bosses", 256, "Badder bosses", false),
    ];

    /// <summary>Every challenge bit together: the largest legal challenge mask.</summary>
    public static int AllMask { get; } = All.Aggregate(0, (mask, entry) => mask | entry.Mask);
}
public enum UpgradeMatch { Any, Exactly, AtLeast }
public enum TierMatch { Any, Exactly, AtLeast, AtMost }
public enum SearchState { Running, Completed, Cancelled, Failed }

/// <summary>
/// One searchable item as the shared catalog lists it.
/// </summary>
/// <param name="SpriteIndex">The item's own cell in <c>items.png</c>. For a ring
/// this is the class's identity cell, which is what a surface with no run to ask
/// — the requirement editor — draws; a scouted ring is drawn in the cell the
/// run's gems give it instead (<see cref="RingGems.SpriteIndex"/>).</param>
/// <param name="TypeIconIndex">The ring's cell in the 8×8 <c>item_icons.png</c>
/// glyph atlas, which is what tells one ring from another on screen; null for
/// everything that is not a ring. The glyph belongs to the class, so unlike the
/// art cell it is the same in every run. Mirrors the Android client's field of
/// the same name, and the catalog's <c>typeIcon</c>.</param>
public sealed record CatalogItem(string Id, string Name, ItemKind Kind, int SpriteIndex, int? Tier,
    WeaponClass? Class = null, int? TypeIconIndex = null);

public enum ScoutItemSource
{
    Heap, Chest, LockedChest, CrystalChest, Tomb, Skeleton, SacrificialFire, Mimic,
    GoldenMimic, CrystalMimic, Statue, ArmoredStatue, Shop, GhostReward,
    WandmakerReward, BlacksmithReward, ImpReward,
    // v4.0.0's Imp vault. The value indexes the wire ids and the document
    // source-name table in ResultsExport, so it stays appended after the
    // v3.3.8 block.
    VaultTreasure
}

/// <summary>
/// The generic Fluent glyph and tint, kept only for wildcard requirements that pin
/// no concrete item and so have no sprite to draw.
/// </summary>
public static partial class KindStyle
{
    public static string Glyph(ItemKind kind) => kind.Family() switch { ItemKind.Weapon => "", ItemKind.Armor => "", ItemKind.Wand => "", _ => "" };
}

public static class Labels
{
    public static string Source(ScoutItemSource value) => value switch
    {
        ScoutItemSource.LockedChest => "Locked chest", ScoutItemSource.CrystalChest => "Crystal chest",
        ScoutItemSource.SacrificialFire => "Sacrificial fire", ScoutItemSource.GoldenMimic => "Golden mimic",
        ScoutItemSource.CrystalMimic => "Crystal mimic", ScoutItemSource.ArmoredStatue => "Armored statue",
        ScoutItemSource.GhostReward => "Ghost reward", ScoutItemSource.WandmakerReward => "Wandmaker reward",
        ScoutItemSource.BlacksmithReward => "Blacksmith reward", ScoutItemSource.ImpReward => "Imp reward",
        ScoutItemSource.VaultTreasure => "Vault treasure",
        _ => string.Concat(value.ToString().Select((c, i) => i > 0 && char.IsUpper(c) ? " " + char.ToLowerInvariant(c) : char.ToLowerInvariant(c).ToString()))
    };
}

/// <summary>
/// Which effects (enchantments, glyphs, curses) a requirement accepts: any
/// effect or none at all, any non-curse effect of the item's family, or one
/// of a chosen set. A plain class so System.Text.Json persists it as-is;
/// <see cref="ItemRequirement.Modifier"/> keeps the pre-set single-name view.
/// </summary>
public sealed class EffectFilter
{
    /// <summary>Every non-curse effect of the family ("any enchantment").</summary>
    public bool AnyEnchantment { get; set; }
    /// <summary>The accepted effect names, in the catalog's order; empty means no restriction.</summary>
    public List<string> Effects { get; set; } = [];

    public static EffectFilter Any() => new();
    public static EffectFilter Enchantment() => new() { AnyEnchantment = true };
    public static EffectFilter OneOf(IEnumerable<string> effects) => new() { Effects = [.. effects] };

    /// <summary>True when the requirement places no condition on the effect.</summary>
    [JsonIgnore] public bool IsAny => !AnyEnchantment && Effects.Count == 0;
    /// <summary>The one chosen effect, or null when the filter is anything else.</summary>
    [JsonIgnore] public string? Single => !AnyEnchantment && Effects.Count == 1 ? Effects[0] : null;

    /// <summary>
    /// Whether this filter lists exactly the family's non-curse effects,
    /// which the document writes as the "any_enchantment" shorthand.
    /// </summary>
    public bool IsEveryEnchantmentOf(ItemKind kind) =>
        !AnyEnchantment && Effects.Count > 0 && Effects.ToHashSet().SetEquals(ItemCatalog.EnchantmentsOf(kind));

    public EffectFilter Clone() => new() { AnyEnchantment = AnyEnchantment, Effects = [.. Effects] };
}

/// <summary>
/// Membership in a combined-level group: the <em>levels</em> of the members of
/// <paramref name="Group"/> (1..LevelSumGroupMax, shown as A..D) must add up to
/// at least <paramref name="AtLeast"/>, where a matched item counts its upgrade
/// plus one. Members are optional, so the group reads "up to N items reaching
/// <paramref name="AtLeast"/> levels" — one +2 ring satisfies a total of 3 on
/// its own, and so does a +0 with a +1. Every member carries the same total.
/// </summary>
public sealed record LevelSum(int Group, int AtLeast);

public sealed partial class ItemRequirement
{
    /// <summary>
    /// The row's name on the requirement board, stable across its edits. A
    /// loaded or imported list is keyed 1…n (<see cref="BoardEditor.Load"/>)
    /// and the shared editor keys the rows it adds; 0 until then. Keys are
    /// the board's own and never part of a query document.
    /// </summary>
    public long Key { get; set; }
    public CatalogItem? Item { get; set; }
    public int Upgrade { get; set; }
    /// <summary>
    /// The single pinned effect, or null — the pre-effect-set view of
    /// <see cref="Effect"/>, kept so saved queries and presets written before
    /// effect sets existed still load (the setter adopts a name; null leaves
    /// the filter alone, so a newer file's <c>Effect</c> is never erased).
    /// </summary>
    public string? Modifier
    {
        get => Effect.Single;
        set { if (value is not null) Effect = EffectFilter.OneOf([value]); }
    }
    /// <summary>Which effects the item may carry.</summary>
    public EffectFilter Effect { get; set; } = EffectFilter.Any();
    public ItemKind Kind { get; set; }
    public int Tier { get; set; }
    public TierMatch TierMatch { get; set; }
    public UpgradeMatch UpgradeMatch { get; set; }
    public ScoutItemSource? Source { get; set; }
    public int? IdentityGroup { get; set; }
    public int? MaximumDepth { get; set; }
    public bool RequireUncursed { get; set; }
    public bool SelectTrinket { get; set; }
    public int TrinketTransmutations { get; set; }
    public int ArtifactTransmutations { get; set; }
    public bool Blanket { get; set; }
    public bool ExcludeResin { get; set; }
    /// <summary>
    /// Requirements sharing a number form one "any of these" slot, satisfied
    /// by any single member. Null for a requirement that stands alone.
    /// </summary>
    public int? AlternativeGroup { get; set; }
    /// <summary>Combined-level group membership; never set on an alternative.</summary>
    public LevelSum? LevelSum { get; set; }
    [JsonIgnore] public string Glyph => KindStyle.Glyph(Kind);
    public ItemRequirement Clone()
    {
        var copy = (ItemRequirement)MemberwiseClone();
        copy.Effect = Effect.Clone();
        return copy;
    }
}

/// <summary>
/// The query's slots and its own checks. The structure between requirements —
/// either/or clusters, stacks, combined levels — is the shared editor's
/// (<see cref="BoardEditor"/>), which folds, edits, words and checks the list;
/// what stays here is how the document and the scout count slots, and the
/// problems of the query's own settings, which the board does not own.
/// </summary>
public static class QueryRelationships
{
    /// <summary>
    /// The requirements grouped into slots, in slot order: a slot sits at its
    /// first member's position and holds every member in requirement order,
    /// as the engine's <c>SearchQuery::slots</c> has it.
    /// </summary>
    public static List<List<ItemRequirement>> Slots(IEnumerable<ItemRequirement> requirements)
    {
        var slots = new List<List<ItemRequirement>>(); var slotOfGroup = new Dictionary<int, List<ItemRequirement>>();
        foreach (var requirement in requirements)
        {
            if (requirement.AlternativeGroup is int group)
            {
                if (!slotOfGroup.TryGetValue(group, out var slot)) { slot = []; slotOfGroup[group] = slot; slots.Add(slot); }
                slot.Add(requirement);
            }
            else slots.Add([requirement]);
        }
        return slots;
    }

    /// <summary>How many slots the query has — what the engine counts as one requirement each.</summary>
    public static int SlotCount(IEnumerable<ItemRequirement> requirements) => Slots(requirements).Count;

    /// <summary>
    /// The first problem of the query's own settings — its Arcane Resin
    /// condition and its floor requirements — or null when they are sound.
    /// The requirement list's problems are the board's
    /// (<see cref="BoardView.Problems"/>; <see cref="BoardEditor.Problem"/>
    /// asks both). The engine only reports a generic rejection over the FFI,
    /// so this runs first.
    /// </summary>
    public static string? Validate(QuerySettings query)
    {
        if (query.ArcaneResin is < 0 or > 65535 || query.ArcaneResinFilter is not { IsValid: true })
            return "Arcane Resin must be 0..65535, with a valid wand floor and source.";
        if (query.FloorRequirements.Any(floor => !floor.IsValid))
            return "Choose a regular floor from 1 through 24 with a feeling or room requirement.";
        if (query.FloorRequirements.Select(floor => floor.Depth).Distinct().Count() != query.FloorRequirements.Count)
            return "Each floor can have only one requirement.";
        if (query.FloorRequirements.FirstOrDefault(floor => floor.Depth > query.MaximumDepth) is { } outside)
            return $"Floor {outside.Depth} exceeds the floor limit of {query.MaximumDepth}.";
        return null;
    }
}

/// <summary>
/// Floor-limit helpers shared by every floor selector. Boss floors 5, 10 and 15
/// generate no searchable items: the engine treats a floor limit of 5/10/15
/// exactly like 4/9/14, so selectors skip them. Floor 20 stays selectable
/// because the Imp shop gives the City boss floor searchable stock.
/// </summary>
public static class FloorLimits
{
    public static readonly int[] EmptyBossFloors = [5, 10, 15];

    /// <summary>Floors offered by floor-limit selectors: 1..MaxDepth minus the empty boss floors.</summary>
    public static readonly int[] Options = Enumerable.Range(1, SearchLimits.MaxDepth).Where(f => !EmptyBossFloors.Contains(f)).ToArray();

    /// <summary>Snaps an empty boss-floor limit to the equivalent floor below it (5→4, 10→9, 15→14).</summary>
    public static int Normalize(int depth) => EmptyBossFloors.Contains(depth) ? depth - 1 : depth;

    /// <summary>The slider index for a floor limit; off-list values snap to the nearest option below (or the first option).</summary>
    public static int IndexOf(int depth)
    {
        var floor = Normalize(depth);
        var exact = Array.IndexOf(Options, floor);
        return exact >= 0 ? exact : Math.Max(0, Array.FindLastIndex(Options, option => option <= floor));
    }
}

/// <summary>
/// The Wandmaker quest a search can demand. Only this giver's variant is worth
/// filtering on: its quest item can be used in the dungeon instead of being
/// handed in. The value orders the picker, with 0 meaning "any".
/// </summary>
public enum WandmakerQuest
{
    Any = 0,
    CorpseDust = 1,
    ElementalEmbers = 2,
    Rotberry = 3,
}

public static class WandmakerQuests
{
    /// <summary>The pickable quests in wire order, "Any" first.</summary>
    public static readonly WandmakerQuest[] All =
    [
        WandmakerQuest.Any,
        WandmakerQuest.CorpseDust,
        WandmakerQuest.ElementalEmbers,
        WandmakerQuest.Rotberry,
    ];

    public static string Label(WandmakerQuest quest) => quest switch
    {
        WandmakerQuest.CorpseDust => "Corpse Dust",
        WandmakerQuest.ElementalEmbers => "Elemental Embers",
        WandmakerQuest.Rotberry => "Rotberry",
        _ => "Any",
    };

    /// <summary>Stable snake_case name used by the shared query document.</summary>
    public static string? DocumentName(WandmakerQuest quest) => quest switch
    {
        WandmakerQuest.CorpseDust => "corpse_dust",
        WandmakerQuest.ElementalEmbers => "elemental_embers",
        WandmakerQuest.Rotberry => "rotberry",
        _ => null,
    };

    public static WandmakerQuest? Named(string name) => name switch
    {
        "corpse_dust" => WandmakerQuest.CorpseDust,
        "elemental_embers" => WandmakerQuest.ElementalEmbers,
        "rotberry" => WandmakerQuest.Rotberry,
        _ => null,
    };
}

public sealed record ArcaneResinFilter(bool Uncursed = true, int? MaximumDepth = null, ScoutItemSource? Source = null, bool IncludeMageWand = false)
{
    public bool IsValid => (MaximumDepth is null or >= 1 and <= SearchLimits.MaxDepth) &&
        (Source is null || Enum.IsDefined(Source.Value));
}

public sealed record FloorRequirement
{
    public static readonly int[] FarmingFloors = [7, 17, 22];
    public int Depth { get; init; }
    public string? Feeling { get; init; }
    public string[] Rooms { get; init; } = [];
    public string[] AnyRooms { get; init; } = [];
    [JsonIgnore]
    public bool IsFarming => FarmingFloors.Contains(Depth) && Feeling == "dark" && Rooms.Length == 0 &&
        AnyRooms.Length == 2 && AnyRooms.Contains("garden") && AnyRooms.Contains("secret_garden");
    [JsonIgnore]
    public bool IsValid => Depth is >= 1 and <= 24 && Depth % 5 != 0 &&
        (Feeling is null or "none" or "chasm" or "water" or "grass" or "dark" or "large" or "traps" or "secrets") &&
        (Feeling is not null || Rooms.Length > 0 || AnyRooms.Length > 0);
    [JsonIgnore]
    public string Summary => string.Join(" · ", new[] { $"Floor {Depth}", Feeling }
        .Concat(Rooms).Append(AnyRooms.Length == 0 ? null : string.Join(" / ", AnyRooms))
        .Where(text => !string.IsNullOrEmpty(text))).Replace('_', ' ');
}

public sealed class QuerySettings
{
    public List<FloorRequirement> FloorRequirements { get; set; } = [];
    public ObservableCollection<ItemRequirement> Requirements { get; set; } = [];
    public int MaximumDepth { get; set; } = SearchLimits.MaxDepth;
    public bool AutoApplyTrinket { get; set; }
    public int ArcaneResin { get; set; }
    public bool ArcaneResinAuto { get; set; }
    [JsonIgnore]
    public bool NeedsResin => ArcaneResinAuto || ArcaneResin > 0;
    public ArcaneResinFilter ArcaneResinFilter { get; set; } = new();
    [System.Text.Json.Serialization.JsonIgnore]
    public bool HasRequirements => Requirements.Count > 0 || NeedsResin || FloorRequirements.Count > 0;
    [System.Text.Json.Serialization.JsonIgnore]
    public int SlotCount => QueryRelationships.SlotCount(Requirements) + FloorRequirements.Count + (NeedsResin ? 1 : 0);
    public bool RequireBlacksmith { get; set; }
    public bool ExcludeBlacksmithRewards { get; set; }
    public WandmakerQuest WandmakerQuest { get; set; } = WandmakerQuest.Any;
    public int Challenges { get; set; }

    public void ToggleFarmingFloor(int depth)
    {
        if (!FloorRequirement.FarmingFloors.Contains(depth)) throw new ArgumentOutOfRangeException(nameof(depth));
        var selected = FloorRequirements.Any(floor => floor.Depth == depth && floor.IsFarming);
        FloorRequirements.RemoveAll(floor => floor.Depth == depth);
        if (!selected)
        {
            FloorRequirements.Add(new() { Depth = depth, Feeling = "dark", AnyRooms = ["garden", "secret_garden"] });
            MaximumDepth = Math.Max(MaximumDepth, depth);
        }
        FloorRequirements.Sort((left, right) => left.Depth.CompareTo(right.Depth));
    }

    public QuerySettings Clone() => new()
    {
        FloorRequirements = FloorRequirements.Select(floor => floor with { Rooms = [.. floor.Rooms], AnyRooms = [.. floor.AnyRooms] }).ToList(),
        Requirements = new ObservableCollection<ItemRequirement>(Requirements.Select(x => x.Clone())),
        MaximumDepth = MaximumDepth,
        AutoApplyTrinket = AutoApplyTrinket,
        ArcaneResin = ArcaneResin,
        ArcaneResinAuto = ArcaneResinAuto,
        ArcaneResinFilter = ArcaneResinFilter with { },
        RequireBlacksmith = RequireBlacksmith,
        ExcludeBlacksmithRewards = ExcludeBlacksmithRewards,
        WandmakerQuest = WandmakerQuest,
        Challenges = Challenges,
    };
}

/// <summary>Every saved seed, its original recipe, and the query that chose it.</summary>
public sealed record TargetRun(QuerySettings Query, IReadOnlyList<string> Seeds,
    IReadOnlyDictionary<string, SeedResult>? Recipes = null,
    IReadOnlyDictionary<string, QuerySettings>? Sources = null)
{
    public static TargetRun Remember(TargetRun? pool, QuerySettings query, IEnumerable<SeedResult> entries)
    {
        var seeds = new List<string>(pool?.Seeds ?? []);
        var known = new HashSet<string>(seeds);
        var recipes = new Dictionary<string, SeedResult>(pool?.Recipes ?? new Dictionary<string, SeedResult>());
        var sources = new Dictionary<string, QuerySettings>(pool?.Sources ?? new Dictionary<string, QuerySettings>());
        foreach (var seed in seeds) sources.TryAdd(seed, pool!.Query);
        var source = query.Clone();
        foreach (var entry in entries) if (known.Add(entry.Seed)) {
            seeds.Add(entry.Seed); recipes[entry.Seed] = entry; sources[entry.Seed] = source;
        }
        return new(pool?.Query ?? source, seeds, recipes, sources);
    }
}

public sealed class QueryPreset
{
    public string Id { get; set; } = Guid.NewGuid().ToString();
    public string Name { get; set; } = "";
    public QuerySettings Query { get; set; } = new();
    [JsonIgnore] public bool IsBuiltIn { get; set; }
}

public static class BuiltInPresets
{
    public static IReadOnlyList<QueryPreset> All { get; } = [
        new()
        {
            Id = "disintegrate", Name = "DISINTEGRATE", IsBuiltIn = true,
            Query = new QuerySettings { AutoApplyTrinket = true, MaximumDepth = 19, Requirements = [
                new() { Kind = ItemKind.Wand, Item = ItemCatalog.Find("wand_disintegration"), Upgrade = 3, UpgradeMatch = UpgradeMatch.AtLeast },
                new() { Kind = ItemKind.Wand, Item = ItemCatalog.Find("wand_disintegration") },
                new() { Kind = ItemKind.Wand, Item = ItemCatalog.Find("wand_disintegration") },
                new() { Kind = ItemKind.Trinket, Item = ItemCatalog.Find("eye_of_newt"), TrinketTransmutations = 1 },
                new() { Kind = ItemKind.Ring, Item = ItemCatalog.Find("ring_energy"), Upgrade = 2, UpgradeMatch = UpgradeMatch.AtLeast },
            ] },
        },
        new()
        {
            Id = "guerilla-assassin", Name = "Guerilla Assassin", IsBuiltIn = true,
            Query = new QuerySettings { AutoApplyTrinket = true, Requirements = [
                new() { Kind = ItemKind.Weapon, Item = ItemCatalog.Find("assassins_blade"), Effect = EffectFilter.OneOf(["Blooming"]),
                    Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly, MaximumDepth = 7 },
                new() { Kind = ItemKind.Armor, Effect = EffectFilter.OneOf(["Camouflage"]) },
                new() { Kind = ItemKind.Ring, Item = ItemCatalog.Find("ring_arcana"), Upgrade = 2, UpgradeMatch = UpgradeMatch.AtLeast },
            ] },
        },
        // Floor 17 as the farming-floor toggle marks it: dark, with a garden.
        new()
        {
            Id = "ring-of-wealth", Name = "Ring of Wealth", IsBuiltIn = true,
            Query = new QuerySettings
            {
                AutoApplyTrinket = false,
                FloorRequirements = [new() { Depth = 17, Feeling = "dark", AnyRooms = ["garden", "secret_garden"] }],
                Requirements = [
                    new() { Kind = ItemKind.Ring, Item = ItemCatalog.Find("ring_wealth"), Upgrade = 4, UpgradeMatch = UpgradeMatch.Exactly },
                    new() { Kind = ItemKind.Artifact, Item = ItemCatalog.Find("dried_rose"), MaximumDepth = 9 },
                    new() { Kind = ItemKind.Armor, Tier = 4, TierMatch = TierMatch.AtMost, Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly, MaximumDepth = 4 },
                    new() { Kind = ItemKind.Weapon, Tier = 4, TierMatch = TierMatch.AtMost, Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly, MaximumDepth = 9 },
                    new() { Kind = ItemKind.Trinket, Item = ItemCatalog.Find("dimensional_sundial"), TrinketTransmutations = 1 },
                ],
            },
        },
        new()
        {
            Id = "necromancer", Name = "Necromancer", IsBuiltIn = true,
            Query = new QuerySettings { AutoApplyTrinket = true, MaximumDepth = 14, WandmakerQuest = WandmakerQuest.CorpseDust, Requirements = [
                new() { Kind = ItemKind.Wand, Item = ItemCatalog.Find("wand_corruption"), Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly },
                new() { Kind = ItemKind.MeleeWeapon, Tier = 5, TierMatch = TierMatch.Exactly, Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly },
                new() { Kind = ItemKind.Armor, Item = ItemCatalog.Find("plate_armor"), Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly },
            ] },
        },
        new()
        {
            Id = "blood-berserker", Name = "Blood Berserker", IsBuiltIn = true,
            Query = new QuerySettings { AutoApplyTrinket = true, Requirements = [
                new() { Kind = ItemKind.MeleeWeapon, Tier = 5, TierMatch = TierMatch.Exactly, Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly,
                    Effect = EffectFilter.OneOf(["Vampiric"]) },
                new() { Kind = ItemKind.Armor, Item = ItemCatalog.Find("plate_armor"), Upgrade = 3, UpgradeMatch = UpgradeMatch.Exactly,
                    Effect = EffectFilter.OneOf(["Thorns"]) },
                new() { Kind = ItemKind.Ring, Item = ItemCatalog.Find("ring_arcana"), Upgrade = 4, UpgradeMatch = UpgradeMatch.Exactly },
                new() { Kind = ItemKind.Artifact, Item = ItemCatalog.Find("chalice_of_blood") },
            ] },
        },
    ];
}

public sealed partial record SeedResult(string Seed, int Number, string? SelectedTrinket = null);
public sealed record ScoutItem(CatalogItem Item, int Depth, int Upgrade, string? Effect, bool Cursed,
    ScoutItemSource Source, byte AccessibilityTag, int AccessibilityGroup, ulong AccessibilityValue,
    bool Secret = false)
{
    // Mirrors transferUpgrade followed by visiblyUpgraded (positive half-up rounding).
    public int DisplayedUpgrade
    {
        get
        {
            var cap = Item.Id switch
            {
                "sandals_of_nature" => 3,
                "ethereal_chains" or "timekeepers_hourglass" => 5,
                _ => 0,
            };
            if (cap == 0) return Upgrade;
            var internalLevel = (Upgrade * cap + 5) / 10;
            return (internalLevel * 10 + cap / 2) / cap;
        }
    }
}
/// <summary>
/// The gems one run gives the twelve ring classes, indexed by class — which is
/// also the class's <see cref="CatalogItem.TypeIconIndex"/>, so a ring looks up
/// its own gem by its glyph.
///
/// Shattered Pixel Dungeon shuffles <c>Ring.gems</c> once per run in
/// <c>Dungeon.init()</c> and hands each ring class the gem at its own index, so
/// which gem — and so which colour — a ring shows is fixed by the seed alone,
/// before any floor is generated and before any challenge is read. The engine
/// reproduces that shuffle and carries it in the <c>SSC3</c> scout packet, which
/// describes the same run; drawing a scouted ring from the catalog cell instead
/// shows the same twelve colours for every seed.
/// </summary>
public sealed record RingGems
{
    /// <summary>Atlas cell of the first ring sprite (<c>ItemSpriteSheet.RINGS</c>).</summary>
    public const int RingSpriteBase = 224;

    /// <summary>How many ring classes — and so gems — there are.</summary>
    public const int Count = 12;

    /// <summary>
    /// The gem ordinal each ring class was given, in catalog ring order. A
    /// shuffle deals every class a distinct gem, so this is always a
    /// permutation of <c>0..11</c>.
    /// </summary>
    public IReadOnlyList<byte> Ordinals { get; }

    /// <summary>
    /// Reads a table from the twelve ordinals a scout packet carries, taking a
    /// copy so a later write to the buffer cannot move a drawn ring.
    /// </summary>
    /// <exception cref="InvalidDataException">The ordinals are not a
    /// permutation of <c>0..11</c>, so they are a corrupt table rather than an
    /// unusual run — the engine's own <c>RingGems::from_ordinals</c> rejects
    /// the same tables.</exception>
    public RingGems(IReadOnlyList<byte> ordinals)
    {
        if (ordinals.Count != Count) throw new InvalidDataException("Unexpected ring gem table size");
        var seen = new bool[Count];
        foreach (var gem in ordinals)
        {
            if (gem >= Count || seen[gem]) throw new InvalidDataException("Ring gems are not a permutation of the twelve gems");
            seen[gem] = true;
        }
        Ordinals = [.. ordinals];
    }

    /// <summary>
    /// The table as it stands before the shuffle: every class holding its own
    /// gem, which is exactly what the catalog's per-ring cells spell out. Only
    /// for surfaces that have no run to ask.
    /// </summary>
    public static RingGems Unshuffled { get; } = new([.. Enumerable.Range(0, Count).Select(gem => (byte)gem)]);

    /// <summary>
    /// The <c>items.png</c> cell <paramref name="item"/> is drawn in during this
    /// run: the run's gem for a ring's class, and the item's own catalog cell for
    /// everything else. Mirrors the engine's <c>sprite_index_in</c>.
    /// </summary>
    public int SpriteIndex(CatalogItem item) =>
        item.TypeIconIndex is int type && type >= 0 && type < Ordinals.Count
            ? RingSpriteBase + Ordinals[type]
            : item.SpriteIndex;
}

/// <param name="Gems">The run's ring gems, which decide what cell each scouted
/// ring is drawn in.</param>
public sealed record ScoutWorld(string Seed, IReadOnlyList<ScoutQuest> Quests, IReadOnlyList<ScoutItem> Items,
    RingGems Gems, IReadOnlyList<CatalogItem>? TrinketOrder = null, IReadOnlyList<ScoutFloorFeeling>? FloorFeelings = null, string? SelectedTrinket = null, ScoutItemMappings? ItemMappings = null,
    IReadOnlyDictionary<int, IReadOnlySet<string>>? FloorRooms = null, IReadOnlyDictionary<int, IReadOnlyList<CatalogItem>>? ArtifactDecks = null)
{
    public bool IsFarmingFloor(int depth) => FloorRequirement.FarmingFloors.Contains(depth)
        && FloorFeelings?.Any(floor => floor.Depth == depth && floor.Feeling == FloorFeeling.Dark) == true
        && FloorRooms?.TryGetValue(depth, out var rooms) == true
        && (rooms.Contains("garden") || rooms.Contains("secret_garden"));
}

public enum FloorFeeling : byte { None, Chasm, Water, Grass, Dark, Large, Traps, Secrets }
public sealed record ScoutFloorFeeling(int Depth, FloorFeeling Feeling);
public sealed record SearchStatus(SearchState State, long Scanned, long Total, long ErrorCode, double Probability)
{
    public bool IsImpossibleQuery => State == SearchState.Completed && Scanned == 0 && Total > 0;
    public bool ProbabilityUnavailable => !double.IsFinite(Probability);
    public string ProbabilityDescription => ProbabilityUnavailable ? "unavailable" : Probability > 0 ? $"{Probability:P4}" : "calculating";
}

/// <summary>
/// The engine's marks for one scouted world: which items satisfy the query,
/// as indices into the scout manifest, and how many of the requirements that
/// selection explains. Produced by <see cref="NativeEngine.ScoutMatches"/>.
/// </summary>
public sealed record ScoutMatches(IReadOnlySet<int> Matched, int MatchedRequirements, int TotalRequirements)
{
    /// <summary>The matched wands consumed as Arcane Resin donors; always a subset of <see cref="Matched"/>.</summary>
    public IReadOnlySet<int> ResinDonors { get; init; } = new HashSet<int>();
    /// <summary>The items the query forbids every requirement to use (Smith rewards while they are excluded); never in <see cref="Matched"/>.</summary>
    public IReadOnlySet<int> Excluded { get; init; } = new HashSet<int>();
    public IReadOnlySet<int> TransmutedTrinkets { get; init; } = new HashSet<int>();
    public IReadOnlySet<(int Depth, int Index)> TransmutedArtifacts { get; init; } = new HashSet<(int, int)>();
}

public static class ItemCatalog
{
    private sealed class Root { public Entry[] Entries { get; set; } = []; public EffectTables Modifiers { get; set; } = new(); }
    private sealed class Entry { public string Id { get; set; } = ""; public string Name { get; set; } = ""; public string Type { get; set; } = ""; public string? Class { get; set; } public int? Tier { get; set; } public int Sprite { get; set; } public int? TypeIcon { get; set; } }
    /// <summary>The upstream effect names, exactly as the shared catalog lists them.</summary>
    private sealed class EffectTables { public string[] WeaponEnchantments { get; set; } = []; public string[] WeaponCurses { get; set; } = []; public string[] ArmorGlyphs { get; set; } = []; public string[] ArmorCurses { get; set; } = []; }
    private static readonly Root Catalog = Load();
    public static IReadOnlyList<CatalogItem> All { get; } = Catalog.Entries.Select(e => new CatalogItem(e.Id, e.Name, Enum.Parse<ItemKind>(e.Type, true), e.Sprite, e.Tier,
        string.IsNullOrEmpty(e.Class) ? null : Enum.Parse<WeaponClass>(e.Class, true), e.TypeIcon)).ToArray();
    // The four effect tables come from the same asset as the items, so a
    // catalog bump carries them and no hand-typed list can fall behind it.
    public static IReadOnlyList<string> Enchantments => Catalog.Modifiers.WeaponEnchantments;
    public static IReadOnlyList<string> WeaponCurses => Catalog.Modifiers.WeaponCurses;
    public static IReadOnlyList<string> Glyphs => Catalog.Modifiers.ArmorGlyphs;
    public static IReadOnlyList<string> ArmorCurses => Catalog.Modifiers.ArmorCurses;
    private static Root Load() =>
        JsonSerializer.Deserialize<Root>(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Assets", "catalog-v4.0.1.json")), new JsonSerializerOptions { PropertyNameCaseInsensitive = true })!;
    public static CatalogItem? Find(string id) => All.FirstOrDefault(x => x.Id == id);
    public static IEnumerable<string> Modifiers(ItemKind kind) => kind.Family() switch { ItemKind.Weapon => Enchantments.Concat(WeaponCurses), ItemKind.Armor => Glyphs.Concat(ArmorCurses), _ => [] };
    /// <summary>The family's non-curse effects: what "any enchantment" stands for.</summary>
    public static IReadOnlyList<string> EnchantmentsOf(ItemKind kind) => kind.Family() switch { ItemKind.Weapon => Enchantments, ItemKind.Armor => Glyphs, _ => [] };
    public static bool IsCurse(ItemKind kind, string effect) => (kind.Family() == ItemKind.Weapon ? WeaponCurses : ArmorCurses).Contains(effect);
}
