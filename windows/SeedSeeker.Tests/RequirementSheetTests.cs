using System.Text.Json.Nodes;
using Xunit;

namespace SeedSeeker.Tests;

/// <summary>
/// The requirement sheet is the shared core's editor, reached through
/// <c>seedfinder_requirement_editor</c>; the core's own tests own its rules —
/// what each control offers and shows, what a change resets, what a save
/// writes and when it may. These cover the bridge the dialog stands on: the
/// golden fixtures' forms read into the typed sheet, the changes the dialog
/// sends are the ones the envelope reads, and the open → change → save flows
/// the dialog runs — requirements, stacks, Arcane Resin — come back through
/// the real engine as the window adopts them.
/// </summary>
public sealed class RequirementSheetTests
{
    private static JsonObject Fixture(string name) => JsonNode.Parse(File.ReadAllText(Path.Combine(
        NativeEngineLibrary.WorkspaceRoot() ?? throw new InvalidOperationException("Could not locate the workspace root."),
        "crates", "seedfinder-core", "tests", "fixtures", "editor", $"{name}.json")))!.AsObject();

    private static SheetForm FormOf(string fixture) => RequirementSheet.Sheet(Fixture(fixture)["response"]!["form"]!);

    private static ItemRequirement Named(string id)
    {
        var item = ItemCatalog.Find(id)!;
        return new() { Kind = item.Kind, Item = item };
    }

    /// <summary>A query holding <paramref name="requirements"/>, taken in as a load would take it.</summary>
    private static QuerySettings Loaded(params ItemRequirement[] requirements)
    {
        var query = new QuerySettings { Requirements = new(requirements) };
        new BoardEditor().Load(query);
        return query;
    }

    /// <summary>Picks the option labelled <paramref name="label"/>, as the dialog's combo does.</summary>
    private static T Option<T>(SheetChoice<T> choice, string label) => choice.Options.Single(option => option.Label == label).Value;

    [Fact]
    public void AStackCountingLevelsReadsIntoItsControls()
    {
        var form = FormOf("editor-open-row");
        Assert.False(form.IsNew);
        Assert.Equal((SheetOrigin.Row, (long?)1), (form.Origin, form.OriginKey));
        Assert.Equal("Ring of Might", form.Title);
        Assert.Equal("ring", form.Kind.Value);
        Assert.Equal("Ring", form.Kind.Options[form.Kind.Selected].Label);
        Assert.Equal(["weapon", "melee_weapon", "thrown_weapon", "armor", "wand", "ring", "trinket", "artifact"], form.Kind.Options.Select(option => option.Value));
        Assert.Equal("ring_might", form.Item.Value);
        Assert.Equal("Any ring", form.Item.Options[0].Label);
        Assert.Null(form.Item.Options[0].Value);
        Assert.Equal("Ring of Might", form.Item.Options[form.Item.Selected].Label);
        // A named ring has no tier, and the combined level speaks for its upgrade.
        Assert.False(form.Tier.Visible);
        Assert.False(form.Upgrade.Visible);
        Assert.False(form.Upgrade.ShowsValue);
        Assert.False(form.Effect.Visible);

        var stack = form.Stack;
        Assert.True(stack.Visible);
        Assert.Equal((2, 1, 3, "×2"), (stack.Count, stack.Min, stack.Max, stack.ValueLabel));
        Assert.True(stack.CountLevels.ShowsValue);
        Assert.Equal((3, 1, 8, "≥ 3 across up to 2"), (stack.CountLevels.Value, stack.CountLevels.Min, stack.CountLevels.Max, stack.CountLevels.ValueLabel));
        Assert.Equal("Count levels together", stack.CountLevels.Label);
        Assert.False(stack.CopyDepth.Visible);

        // A floor slider runs over the floors that hold items, and sits on its value.
        var floor = form.FloorLimit;
        Assert.True(floor.Visible);
        Assert.False(floor.ShowsValue);
        Assert.DoesNotContain(floor.Options, option => option.Value is 5 or 10 or 15);
        Assert.Equal(4, floor.Options[floor.Selected].Value);
        Assert.Equal(4, floor.At(floor.Selected));
        Assert.Equal((1, 6, 24), (floor.At(-3), floor.At(4.4), floor.At(99)));
        Assert.Equal(("Limit this item to a floor", "Within first 4 floors"), (floor.Label, floor.ValueLabel));
        Assert.Equal("Require uncursed", form.Uncursed.Label);
        Assert.Null(form.Source.Value);
        Assert.Equal(0, form.Source.Selected);
        Assert.Contains(form.Source.Options, option => option is { Value: "locked_chest", Label: "Locked chest" });
        Assert.False(form.Resin.Visible);
        Assert.Empty(form.Errors);
        Assert.True(form.CanSave);
        Assert.Equal("Ring of Might", form.Preview!.Name);
        Assert.Equal(0, form.Preview.Key);
    }

    [Fact]
    public void TheSheetsOtherFixturesReadIntoTheirControls()
    {
        // Weapons are listed under their tiers, the wildcard first.
        var fresh = FormOf("editor-open-new");
        Assert.True(fresh.IsNew);
        Assert.Equal(SheetOrigin.New, fresh.Origin);
        Assert.Equal("Any weapon", fresh.Item.Options[0].Label);
        Assert.Equal("Tier 2", fresh.Item.Options[1].Group);
        Assert.True(fresh.Tier.Visible);
        Assert.False(fresh.Tier.ShowsValue);
        Assert.Equal(["Any", "Exactly", "At least", "At most"], fresh.Tier.Modes.Select(mode => mode.Label));
        Assert.Equal(0, fresh.Tier.Picker.Selected);
        Assert.False(fresh.Tier.Picker.SameOptions(fresh.Upgrade.Picker));
        Assert.Equal((3, 2, 5, "Tier 3"), (fresh.Tier.Value, fresh.Tier.Min, fresh.Tier.Max, fresh.Tier.ValueLabel));
        Assert.True(fresh.Effect.Visible);
        Assert.False(fresh.Effect.ShowsChoices);
        Assert.Equal("Enchantments", fresh.Effect.Heading(curse: false));
        Assert.Equal("Curses", fresh.Effect.Heading(curse: true));
        Assert.Contains(fresh.Effect.Choices, choice => choice.Curse);

        var ticked = FormOf("editor-change-effect");
        Assert.True(ticked.Effect.ShowsChoices);
        Assert.Equal(["Blazing"], ticked.Effect.Choices.Where(choice => choice.Selected).Select(choice => choice.Value));
        Assert.Equal("Matches any one of 1 effect.", ticked.Effect.Caption);
        Assert.True(ticked.Effect.SameChoices(fresh.Effect));

        var copies = FormOf("editor-change-count");
        Assert.True(copies.Stack.CopyDepth.ShowsValue);
        Assert.Equal(6, copies.Stack.CopyDepth.Value);
        Assert.Equal("Copies within first 6 floors", copies.Stack.CopyDepth.ValueLabel);
        Assert.False(copies.Stack.CountLevels.Visible);

        var blanket = FormOf("editor-open-blanket");
        Assert.True(blanket.Blanket);
        Assert.Equal("melee_weapon", blanket.Kind.Value);
        Assert.False(blanket.Stack.Visible);

        var duplicate = FormOf("editor-save-refused");
        Assert.False(duplicate.CanSave);
        Assert.Equal(["This trinket is already required. Each trinket appears only once in the deck."], duplicate.Errors);
        Assert.Null(duplicate.Preview);
        Assert.Equal("Rat Skull", duplicate.Title);
        Assert.False(duplicate.Uncursed.Visible);
    }

    [Fact]
    public void TheResinSheetEditsTheQuerysResinWithTheSheetsOwnFilters()
    {
        var form = FormOf("editor-resin-open");
        Assert.Equal(SheetOrigin.Resin, form.Origin);
        Assert.False(form.IsNew);
        Assert.True(form.ResinPicked);
        Assert.Equal("Arcane Resin", form.Title);
        Assert.Equal("arcane_resin", form.Item.Value);
        Assert.Equal("Arcane Resin", form.Item.Options[form.Item.Selected].Label);
        Assert.Equal(new SheetResin(true, true, 2, true), form.Resin);
        Assert.Equal("Require uncursed wands", form.Uncursed.Label);
        Assert.False(form.Uncursed.Value);
        Assert.Equal("chest", form.Source.Value);
        Assert.Equal("Limit wands to a floor", form.FloorLimit.Label);
        Assert.Null(form.Preview);
        Assert.False(form.ExcludeResin.Visible);

        var empty = FormOf("editor-resin-amount-invalid");
        Assert.Null(empty.Resin.Amount);
        Assert.Equal(["Enter an amount from 1 to 65535."], empty.Errors);
        Assert.False(empty.CanSave);
    }

    /// <summary>The sheet's bounds are the ones the engine publishes, which no local copy has to follow.</summary>
    [Fact]
    public void TheSheetsBoundsAreTheEnginesLimits()
    {
        var limits = (JsonObject)JsonNode.Parse(NativeEngine.EngineInfoJson())!["limits"]!;
        int Limit(string key) => (int)limits[key]!;
        var query = Loaded();
        var sheet = RequirementSheet.Open(query, null);
        sheet.Change(SheetChange.SetTierMode("exact"));
        Assert.Equal((Limit("exactTierMin"), Limit("exactTierMax")), (sheet.Form.Tier.Min, sheet.Form.Tier.Max));
        sheet.Change(SheetChange.SetTierMode("at_most"));
        Assert.Equal((Limit("boundedTierMin"), Limit("boundedTierMax")), (sheet.Form.Tier.Min, sheet.Form.Tier.Max));
        Assert.Equal(Limit("stackMax"), sheet.Form.Stack.Max);
        Assert.Equal(Limit("maxDepth"), sheet.Form.FloorLimit.Options[^1].Value);
        sheet.Change(SheetChange.SetKind("trinket"));
        Assert.Equal(Limit("trinketTransmutationsMax"), sheet.Form.Transmutations.Max);
        sheet.Change(SheetChange.SetKind("artifact"));
        Assert.Equal(Limit("artifactTransmutationsMax"), sheet.Form.Transmutations.Max);
    }

    [Fact]
    public void ChangesAreWrittenAsTheEnvelopeReadsThem()
    {
        Assert.Equal("""{"type":"set_item","value":null}""", SheetChange.SetItem(null).ToString());
        Assert.Equal("""{"type":"set_item","value":"arcane_resin"}""", SheetChange.SetItem("arcane_resin").ToString());
        Assert.Equal("""{"type":"set_kind","value":"thrown_weapon"}""", SheetChange.SetKind("thrown_weapon").ToString());
        Assert.Equal("""{"type":"set_tier","value":255}""", SheetChange.SetTier(300).ToString());
        Assert.Equal("""{"type":"set_count_levels","value":true}""", SheetChange.SetCountLevels(true).ToString());
        Assert.Equal("""{"type":"set_resin_amount","value":null}""", SheetChange.SetResinAmount(double.NaN).ToString());
        Assert.Equal("""{"type":"set_resin_amount","value":2.5}""", SheetChange.SetResinAmount(2.5).ToString());
        Assert.Equal("""{"type":"set_source","value":null}""", SheetChange.SetSource(null).ToString());
        // The fixture's own change, as the dialog would send it.
        var fixture = Fixture("editor-change-item");
        Assert.True(JsonNode.DeepEquals(fixture["request"]!["change"], JsonNode.Parse(SheetChange.SetItem("spear").ToString())));
    }

    [Fact]
    public void ANewChipIsBuiltAndSavedThroughTheEngine()
    {
        var query = Loaded(new ItemRequirement { Kind = ItemKind.Wand });
        var sheet = RequirementSheet.Open(query, null);
        Assert.True(sheet.Form.IsNew);
        Assert.Equal("weapon", sheet.Form.Kind.Value);
        var items = sheet.Form.Item;

        sheet.Change(SheetChange.SetKind("melee_weapon"));
        sheet.Change(SheetChange.SetTierMode("at_least"));
        Assert.True(sheet.Form.Tier.ShowsValue);
        Assert.Equal((3, 3, 4), (sheet.Form.Tier.Value, sheet.Form.Tier.Min, sheet.Form.Tier.Max));
        sheet.Change(SheetChange.SetTier(4));
        Assert.Equal("Tier 4 or higher", sheet.Form.Tier.ValueLabel);
        sheet.Change(SheetChange.SetItem(Option(sheet.Form.Item, "Spear")));
        Assert.Equal("spear", sheet.Form.Item.Value);
        Assert.False(items.SameOptions(sheet.Form.Item));
        // A named item is the tier it is: the tier control goes, and moving it changes nothing.
        Assert.False(sheet.Form.Tier.Visible);
        var named = sheet.Form;
        sheet.Change(SheetChange.SetTier(5));
        Assert.Equal((named.Tier.Mode, named.Tier.Value), (sheet.Form.Tier.Mode, sheet.Form.Tier.Value));
        Assert.Equal(named.Title, sheet.Form.Title);
        Assert.True(named.Item.SameOptions(sheet.Form.Item));

        sheet.Change(SheetChange.SetUpgradeMode("exact"));
        sheet.Change(SheetChange.SetUpgrade(3));
        sheet.Change(SheetChange.SetEffectMode("specific"));
        sheet.Change(SheetChange.ToggleEffect("Blazing"));
        sheet.Change(SheetChange.SetCount(2));
        sheet.Change(SheetChange.SetCopyDepthEnabled(true));
        // The copies' floor slider steps over the empty boss floor.
        sheet.Change(SheetChange.SetCopyDepth(5));
        Assert.Equal(6, sheet.Form.Stack.CopyDepth.Value);
        Assert.Equal("Spear", sheet.Form.Preview!.Name);

        var saved = sheet.Save(query)!;
        Assert.NotNull(saved.Rows);
        Assert.Null(saved.Resin);
        Assert.False(saved.ClearResin);
        Assert.True(saved.ApplyTo(query));
        Assert.Equal(3, query.Requirements.Count);
        var spear = query.Requirements.Single(row => row.Key == saved.Focus);
        Assert.Equal(("spear", UpgradeMatch.Exactly, 3, "Blazing"), (spear.Item?.Id, spear.UpgradeMatch, spear.Upgrade, spear.Modifier));
        var entry = new BoardEditor().View(query).EntryOf(spear.Key)!;
        Assert.Equal((2, (int?)6), (entry.Stack.Count, entry.Stack.CopyDepth));
        Assert.True(query.Requirements.Select(row => row.Key).Distinct().Count() == 3);

        // Reopened, the chip shows what was saved; saved as it is, it changes nothing.
        var again = RequirementSheet.Open(query, spear.Key);
        Assert.Equal((SheetOrigin.Row, (long?)spear.Key), (again.Form.Origin, again.Form.OriginKey));
        Assert.Equal((2, 6), (again.Form.Stack.Count, again.Form.Stack.CopyDepth.Value));
        Assert.Equal("+3", again.Form.Upgrade.ValueLabel);
        var rows = query.Requirements;
        var unchanged = again.Save(query)!;
        Assert.Null(unchanged.Rows);
        Assert.Equal(spear.Key, unchanged.Focus);
        Assert.False(unchanged.ApplyTo(query));
        Assert.Same(rows, query.Requirements);
    }

    [Fact]
    public void ASaveTheEditorRefusesKeepsTheSheetOpenWithItsReasons()
    {
        var query = Loaded(Named("rat_skull"));
        var sheet = RequirementSheet.Open(query, null);
        sheet.Change(SheetChange.SetKind("trinket"));
        sheet.Change(SheetChange.SetItem("rat_skull"));
        Assert.False(sheet.Form.CanSave);
        Assert.Equal(["This trinket is already required. Each trinket appears only once in the deck."], sheet.Form.Errors);
        Assert.Null(sheet.Save(query));
        Assert.Equal(["This trinket is already required. Each trinket appears only once in the deck."], sheet.Form.Errors);
        Assert.Single(query.Requirements);
        // The sheet goes on from the refused draft.
        sheet.Change(SheetChange.SetItem("mimic_tooth"));
        Assert.True(sheet.Form.CanSave);
        Assert.True(sheet.Save(query)!.ApplyTo(query));
        Assert.Equal(["rat_skull", "mimic_tooth"], query.Requirements.Select(row => row.Item?.Id));
    }

    [Fact]
    public void AnImportedItemTheFreshListHidesStaysListedAndRoundTrips()
    {
        var query = ResultsExport.DecodeQueryDocument("""{"requirements":[{"kind":"melee_weapon","item":"worn_shortsword","upgrade":2}]}""");
        new BoardEditor().Load(query);
        var sheet = RequirementSheet.Open(query, query.Requirements[0].Key);
        var worn = sheet.Form.Item.Options[sheet.Form.Item.Selected];
        Assert.Equal(("worn_shortsword", true), (worn.Value, worn.Hidden));
        Assert.Single(sheet.Form.Item.Options, option => option.Hidden);
        Assert.Null(sheet.Save(query)!.Rows);
        // A fresh sheet never offers it.
        Assert.DoesNotContain(RequirementSheet.Open(query, null).Form.Item.Options, option => option.Value == "worn_shortsword");
    }

    [Fact]
    public void PickingArcaneResinSavesTheQuerysResin()
    {
        var query = Loaded(new ItemRequirement { Kind = ItemKind.Wand, UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 3 });
        var sheet = RequirementSheet.Open(query, null, offerResin: true);
        sheet.Change(SheetChange.SetKind("wand"));
        Assert.Equal("Arcane Resin", sheet.Form.Item.Options[1].Label);
        sheet.Change(SheetChange.SetItem("arcane_resin"));
        Assert.True(sheet.Form.ResinPicked);
        Assert.Equal(new SheetResin(true, false, 2, false), sheet.Form.Resin);
        // An emptied amount cannot save; the typed one can.
        sheet.Change(SheetChange.SetResinAmount(double.NaN));
        Assert.Equal(["Enter an amount from 1 to 65535."], sheet.Form.Errors);
        Assert.Null(sheet.Save(query));
        sheet.Change(SheetChange.SetResinAmount(5));
        sheet.Change(SheetChange.SetIncludeMageWand(true));
        sheet.Change(SheetChange.SetFloorLimitEnabled(true));
        sheet.Change(SheetChange.SetSource("locked_chest"));
        Assert.True(sheet.Form.CanSave);

        var saved = sheet.Save(query)!;
        Assert.Null(saved.Rows);
        Assert.Null(saved.Focus);
        Assert.Equal(new ResinCondition(false, 5, new ArcaneResinFilter(true, 4, ScoutItemSource.LockedChest, true)), saved.Resin);
        Assert.True(saved.ApplyTo(query));
        Assert.Equal((false, 5), (query.ArcaneResinAuto, query.ArcaneResin));
        Assert.Equal(new ArcaneResinFilter(true, 4, ScoutItemSource.LockedChest, true), query.ArcaneResinFilter);
        Assert.Single(query.Requirements);

        // A blanket sheet never offers the query's resin.
        var blanket = RequirementSheet.Open(query, null, blanket: true, offerResin: true);
        blanket.Change(SheetChange.SetKind("wand"));
        Assert.DoesNotContain(blanket.Form.Item.Options, option => option.Value == "arcane_resin");
    }

    [Fact]
    public void TheResinChipOpensOnTheQuerysResinAndCanBecomeAWand()
    {
        var query = Loaded(Named("rat_skull"));
        query.ArcaneResinAuto = true;
        query.ArcaneResinFilter = new(false, 9, null, true);
        var sheet = RequirementSheet.Open(query, null, openResin: true);
        Assert.Equal(SheetOrigin.Resin, sheet.Form.Origin);
        Assert.Equal(new SheetResin(true, true, 2, true), sheet.Form.Resin);
        Assert.False(sheet.Form.Uncursed.Value);
        Assert.True(sheet.Form.FloorLimit.ShowsValue);
        Assert.Equal(9, sheet.Form.FloorLimit.Value);

        // Saved as it opened, the resin stays as it was.
        var kept = RequirementSheet.Open(query, null, openResin: true).Save(query)!;
        Assert.Equal(new ResinCondition(true, 0, new(false, 9, null, true)), kept.Resin);
        Assert.False(kept.ApplyTo(query));

        // Saved as a wand, the chip joins the board and the resin goes.
        sheet.Change(SheetChange.SetItem("wand_frost"));
        Assert.False(sheet.Form.ResinPicked);
        Assert.Equal("Wand of Frost", sheet.Form.Title);
        var saved = sheet.Save(query)!;
        Assert.True(saved.ClearResin);
        Assert.True(saved.ApplyTo(query));
        Assert.False(query.NeedsResin);
        Assert.Equal(new ArcaneResinFilter(), query.ArcaneResinFilter);
        Assert.Equal("wand_frost", query.Requirements.Single(row => row.Key == saved.Focus).Item?.Id);
    }

    [Fact]
    public void AWandChipTurnedIntoResinLeavesTheBoard()
    {
        var query = Loaded(new ItemRequirement { Kind = ItemKind.Wand }, Named("rat_skull"));
        var wand = query.Requirements[0].Key;
        var sheet = RequirementSheet.Open(query, wand, offerResin: true);
        sheet.Change(SheetChange.SetItem("arcane_resin"));
        var saved = sheet.Save(query)!;
        Assert.Equal(2, saved.Resin!.Amount);
        Assert.True(saved.ApplyTo(query));
        Assert.DoesNotContain(query.Requirements, row => row.Key == wand);
        Assert.Equal(2, query.ArcaneResin);
    }

    [Fact]
    public void ABlanketSheetStartsFromTheFirstOrdinaryKind()
    {
        var query = Loaded(new ItemRequirement { Kind = ItemKind.ThrownWeapon, UpgradeMatch = UpgradeMatch.Exactly, Upgrade = 2 });
        var sheet = RequirementSheet.Open(query, null, blanket: true);
        Assert.True(sheet.Form.Blanket);
        Assert.Equal("thrown_weapon", sheet.Form.Kind.Value);
        var saved = sheet.Save(query)!;
        Assert.True(saved.ApplyTo(query));
        Assert.True(query.Requirements[1].Blanket);
        Assert.Equal(ItemKind.ThrownWeapon, query.Requirements[1].Kind);
    }
}
