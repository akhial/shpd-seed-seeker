//! The requirement sheet: the dialog a chip opens into, as a value.
//!
//! Every platform grew its own sheet, and each carried its own copy of the
//! rules behind it: what switching the category resets, which item the
//! picker offers, where a floor slider lands on an empty boss floor, when the
//! combined-level switch appears, what a save writes and whether it may. The
//! copies drifted — a category switch that kept a stale resin exclusion on
//! one platform, an at-least slider that could not reach +0 on another, a
//! save that broke a stack everywhere but Linux. Here the sheet is data:
//!
//! - [`open`] builds a [`Draft`] from the rows and the chip tapped (or none,
//!   for a new chip, or the resin chip);
//! - [`change`] applies one [`Change`] — a control the user moved — and
//!   returns the next draft;
//! - [`form`] says what the sheet shows: every control's visibility, value,
//!   range, options and words, the preview chip, and the errors;
//! - [`save`] turns the draft into the board edit ([`Edit::Save`]) and the
//!   query's Arcane Resin outcome, or refuses with the reasons.
//!
//! Platforms keep the drawing and the dialog chrome — titles and button
//! words follow [`Form::mode`], [`Form::origin`], [`Form::blanket`] and
//! [`Form::resin_picked`]. The rules follow the web's `RequirementEditor`,
//! with the shared design's settled choices: at-least bounds of +1…max−1
//! (a loaded "+0 or higher" opens as any upgrade, "+max or higher" as
//! exactly +max), floor sliders that step over empty boss floors, a resin
//! filter kept apart from the wand draft, and a save guard that refuses a
//! save which would newly break the list around the saved row.

use crate::artifacts::TRANSMUTATION_COUNT as ARTIFACT_TRANSMUTATIONS;
use crate::catalog::{
    ALL_ARMOR_EFFECTS, ALL_WEAPON_EFFECTS, Effect, ITEMS, ItemDefinition, ItemId, ItemKind,
    WeaponCategory, item,
};
use crate::main_world::{EMPTY_BOSS_FLOORS, normalize_floor_limit};
use crate::model::ItemSource;
use crate::query::{
    ARCANE_RESIN_MAX, ARCANE_RESIN_MIN, ArcaneResinFilter, BOUNDED_TIER_MAX, BOUNDED_TIER_MIN,
    EXACT_TIER_MAX, EXACT_TIER_MIN, EffectRequirement, EffectSet, MAX_SEARCH_DEPTH, Requirement,
    TierRequirement, UpgradeRequirement,
};
use crate::trinkets::TRANSMUTATION_COUNT as TRINKET_TRANSMUTATIONS;

use super::board::{ChipStack, Edit, EditResult, HeldLabels, apply_holding, board_items};
use super::chips::{ChipView, ResinAmount, ResinState, board_view};
use super::labels::{
    ARCANE_RESIN, KindName, RESIN_AMOUNT, RESIN_AUTO, RESIN_AUTO_CAPTION, RESIN_MAGE_WAND,
    RESIN_MAGE_WAND_CAPTION, RESIN_MINIMUM, category_label, count_text, requirement_title,
    weapon_type_label, wildcard_label,
};
use super::problems::{Problem, problems, row_problems};
use super::stack::{level_capacity, stack_view};
use super::{Row, STACK_MAX, is_valid_key, mint_key, repair_keys, skip_boss_floor};

/// The [`Draft::v`] this build writes. A platform stores a draft as an
/// opaque string between calls; a draft of another version cannot be read,
/// and the platform reopens the sheet from its rows instead.
pub const DRAFT_VERSION: u8 = 1;

/// The draft error of a trinket some other ordinary row already names: the
/// deck holds each trinket once, so two requirements can never both match.
pub const DUPLICATE_TRINKET: &str =
    "This trinket is already required. Each trinket appears only once in the deck.";

/// The draft error of a resin amount that is not a whole number in range —
/// including an empty field, which the platforms send as no amount. The
/// range is the query format's ([`ARCANE_RESIN_MIN`], [`ARCANE_RESIN_MAX`]),
/// and so the form's (`ResinControl::min`, `ResinControl::max`).
pub const RESIN_AMOUNT_RANGE: &str = "Enter an amount from 1 to 65535.";

/// The tier a tier slider starts from before one was chosen (web default).
const DEFAULT_TIER: u8 = 3;

/// The upgrade an upgrade slider starts from before one was chosen.
const DEFAULT_UPGRADE: u8 = 1;

/// The floor a floor-limit switch turns on at, for the item, its copies and
/// the resin donors alike.
const DEFAULT_FLOOR: u8 = 4;

/// The resin amount a sheet offers when the query asks for none yet.
const DEFAULT_RESIN_AMOUNT: f64 = 2.0;

/// Where a sheet came from, which decides what saving it writes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Origin {
    /// A chip that is not on the board yet.
    New,
    /// The visible row with this key.
    Row(u64),
    /// The Arcane Resin chip: the query's resin condition, not a row.
    /// Saving it as anything but resin clears the query's resin.
    Resin,
}

/// What the sheet adds or edits, as the platforms title it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormMode {
    New,
    Edit,
}

/// The item picker's choice.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ItemChoice {
    /// The wildcard: any item of the kind.
    Any,
    Item(ItemId),
    /// The query's Arcane Resin condition, offered among the wands: resin is
    /// what surplus wands melt into.
    ArcaneResin,
}

/// The tier filter's predicate, apart from its value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TierMode {
    Any,
    Exact,
    AtLeast,
    AtMost,
}

impl TierMode {
    /// Every mode, in the order the tier control lists them.
    pub const ALL: [Self; 4] = [Self::Any, Self::Exact, Self::AtLeast, Self::AtMost];

    /// The mode of a tier filter.
    #[must_use]
    pub const fn of(tier: TierRequirement) -> Self {
        match tier {
            TierRequirement::Any => Self::Any,
            TierRequirement::Exact(_) => Self::Exact,
            TierRequirement::AtLeast(_) => Self::AtLeast,
            TierRequirement::AtMost(_) => Self::AtMost,
        }
    }

    /// The mode control's word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Exact => "Exactly",
            Self::AtLeast => "At least",
            Self::AtMost => "At most",
        }
    }

    /// The tiers the mode may name: 2…5 exactly, 3…4 as a bound (the ends
    /// would be redundant with "any" or "exactly"). "Any" keeps the exact
    /// range for the value it remembers.
    #[must_use]
    pub const fn range(self) -> (u8, u8) {
        match self {
            Self::Any | Self::Exact => (EXACT_TIER_MIN, EXACT_TIER_MAX),
            Self::AtLeast | Self::AtMost => (BOUNDED_TIER_MIN, BOUNDED_TIER_MAX),
        }
    }

    const fn filter(self, tier: u8) -> TierRequirement {
        match self {
            Self::Any => TierRequirement::Any,
            Self::Exact => TierRequirement::Exact(tier),
            Self::AtLeast => TierRequirement::AtLeast(tier),
            Self::AtMost => TierRequirement::AtMost(tier),
        }
    }
}

/// The upgrade filter's predicate, apart from its value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UpgradeMode {
    Any,
    Exact,
    AtLeast,
}

impl UpgradeMode {
    /// Every mode, in the order the upgrade control lists them.
    pub const ALL: [Self; 3] = [Self::Any, Self::Exact, Self::AtLeast];

    /// The mode of an upgrade filter.
    #[must_use]
    pub const fn of(upgrade: UpgradeRequirement) -> Self {
        match upgrade {
            UpgradeRequirement::Any => Self::Any,
            UpgradeRequirement::Exact(_) => Self::Exact,
            UpgradeRequirement::AtLeast(_) => Self::AtLeast,
        }
    }

    /// The mode control's word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Any => "Any",
            Self::Exact => "Exactly",
            Self::AtLeast => "At least",
        }
    }

    const fn filter(self, upgrade: u8) -> UpgradeRequirement {
        match self {
            Self::Any => UpgradeRequirement::Any,
            Self::Exact => UpgradeRequirement::Exact(upgrade),
            Self::AtLeast => UpgradeRequirement::AtLeast(upgrade),
        }
    }
}

/// The effect control's mode. "Specific…" with nothing ticked yet is a
/// state of the sheet, not a filter — it saves as any effect — which is why
/// the draft carries the mode beside the requirement.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EffectMode {
    Any,
    /// Every enchantment (or glyph), no curse.
    AnyEnchantment,
    Specific,
}

impl EffectMode {
    /// Every mode, in the order the effect control lists them.
    pub const ALL: [Self; 3] = [Self::Any, Self::AnyEnchantment, Self::Specific];

    /// The mode control's word; armor says "Any glyph".
    #[must_use]
    pub const fn label(self, family: ItemKind) -> &'static str {
        match (self, family) {
            (Self::Any, _) => "Any",
            (Self::AnyEnchantment, ItemKind::Armor) => "Any glyph",
            (Self::AnyEnchantment, _) => "Any enchantment",
            (Self::Specific, _) => "Specific…",
        }
    }
}

/// Which heading an effect choice sits under.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EffectGroup {
    /// The family's enchantments (weapons) or glyphs (armor).
    Enchantment,
    Curse,
}

impl EffectGroup {
    /// The heading: `Enchantments` (`Glyphs` on armor) or `Curses`.
    #[must_use]
    pub const fn label(self, family: ItemKind) -> &'static str {
        match (self, family) {
            (Self::Enchantment, ItemKind::Armor) => "Glyphs",
            (Self::Enchantment, _) => "Enchantments",
            (Self::Curse, _) => "Curses",
        }
    }
}

/// The effect section's label, as every platform titled it: `Enchantment`,
/// or `Glyph` on armor.
const fn effect_section_label(family: ItemKind) -> &'static str {
    match family {
        ItemKind::Armor => "Glyph",
        _ => "Enchantment",
    }
}

/// The resin section's values. They live apart from the wand draft: picking
/// Arcane Resin and going back to a wand leaves the wand as it was, and the
/// resin filter starts from the query's own rather than from whatever the
/// wand draft said (a new wand is not "uncursed", but the resin filter is by
/// default).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResinDraft {
    /// Enough resin for the kept wands, rather than a fixed amount.
    pub auto: bool,
    /// The amount as typed: `None` for an empty field. Only whole numbers
    /// from 1 to 65535 save ([`RESIN_AMOUNT_RANGE`]).
    pub amount: Option<f64>,
    pub include_mage_wand: bool,
    pub uncursed: bool,
    pub max_depth: Option<u8>,
    pub source: Option<ItemSource>,
}

impl Default for ResinDraft {
    /// Amount 2, uncursed donors, no other filter — the engine's default
    /// filter.
    fn default() -> Self {
        Self::seeded(None)
    }
}

impl ResinDraft {
    /// The values a sheet starts from: the query's resin condition, or the
    /// defaults. An Auto query still offers 2 once the amount is chosen.
    #[must_use]
    pub fn seeded(resin: Option<&ResinState>) -> Self {
        let filter = resin.map_or_else(ArcaneResinFilter::default, |resin| resin.filter);
        let (auto, amount) = match resin.map(|resin| resin.amount) {
            Some(ResinAmount::AtLeast(amount)) => (false, f64::from(amount)),
            Some(ResinAmount::Auto) => (true, DEFAULT_RESIN_AMOUNT),
            None => (false, DEFAULT_RESIN_AMOUNT),
        };
        Self {
            auto,
            amount: Some(amount),
            include_mage_wand: filter.include_mage_wand,
            uncursed: filter.uncursed,
            max_depth: filter.max_depth.map(floor_value),
            source: filter.source,
        }
    }

    /// The resin condition these values save as, or `None` while the amount
    /// is not a whole number from 1 to 65535.
    #[must_use]
    pub fn state(&self) -> Option<ResinState> {
        let amount = if self.auto {
            ResinAmount::Auto
        } else {
            ResinAmount::AtLeast(whole_amount(self.amount?)?)
        };
        Some(ResinState {
            amount,
            filter: ArcaneResinFilter {
                include_mage_wand: self.include_mage_wand,
                uncursed: self.uncursed,
                max_depth: self.max_depth.map(floor_value),
                source: self.source,
            },
        })
    }
}

/// `amount` as a resin amount, when it is a whole number the query format
/// holds ([`ARCANE_RESIN_MIN`] to [`ARCANE_RESIN_MAX`]).
#[allow(
    clippy::float_cmp, // An exact whole-number test is the point.
    clippy::cast_possible_truncation, // Range-checked just before.
    clippy::cast_sign_loss
)]
fn whole_amount(amount: f64) -> Option<u16> {
    ((f64::from(ARCANE_RESIN_MIN)..=f64::from(ARCANE_RESIN_MAX)).contains(&amount)
        && amount.trunc() == amount)
        .then_some(amount as u16)
}

/// Everything an open sheet holds between calls. A platform keeps it as it
/// comes back from [`open`] and [`change`] and hands it to [`form`] and
/// [`save`]; it never edits the fields itself.
///
/// [`Draft::requirement`] is authoritative: it is what a save stores. The
/// other fields hold what a requirement cannot — a stack's shape, values a
/// control remembers while it is switched off, and the resin section.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent facts about one sheet.
pub struct Draft {
    /// [`DRAFT_VERSION`].
    pub v: u8,
    pub origin: Origin,
    /// The row a save writes: the edited row, a key a platform claimed for
    /// a new one, or `None` to mint one.
    pub key: Option<u64>,
    /// The requirement being edited. It keeps its alternative group (a
    /// cluster member stays in its cluster); stack and combined-level labels
    /// are the save's to write, from `count` and `total`.
    pub requirement: Requirement,
    /// The tier the tier control shows while the filter is "any".
    pub tier_value: u8,
    /// The upgrade the upgrade control shows while the filter is "any".
    pub upgrade_value: u8,
    pub effect_mode: EffectMode,
    /// How many items the chip asks for, its anchor included.
    pub count: u8,
    /// The stack's combined level, when it counts levels together.
    pub total: Option<u8>,
    /// The floor limit of the stack's extra copies.
    pub copy_depth: Option<u8>,
    /// The floor a floor-limit switch turns back on at.
    pub floor_limit_memory: u8,
    /// The floor the copy floor-limit switch turns back on at.
    pub copy_depth_memory: u8,
    /// The count the transmutation switch turns back on at.
    pub transmutations_memory: u8,
    /// A cluster member: it keeps its cluster, and its stack cannot count
    /// levels.
    pub in_cluster: bool,
    pub blanket: bool,
    /// Whether the item picker offers Arcane Resin among the wands (the
    /// platform lets the sheet set the query's resin).
    pub offer_resin: bool,
    /// The trinkets other ordinary rows name, for [`DUPLICATE_TRINKET`].
    pub taken_trinkets: Vec<ItemId>,
    /// Arcane Resin is the picked item: the sheet edits [`Draft::resin`].
    pub resin_picked: bool,
    pub resin: ResinDraft,
    /// The query's resin condition the sheet was opened with, which a resin
    /// sheet saved untouched leaves as it is.
    pub query_resin: Option<ResinState>,
    /// The rows the sheet was opened on: the save guard runs the save on
    /// them to find what it would break.
    pub rows: Vec<Row>,
}

/// One control the user moved. A change to a control the [`Form`] does not
/// show — a tier on a named item, a blanket's stack — changes nothing, so
/// a stale gesture can never write a field the sheet hides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// The category picker. Picking the category already chosen keeps
    /// everything; another resets what does not carry over (web `setKind`).
    SetCategory(ItemKind),
    /// Any, melee or thrown, on a weapon. The item stays only if it is one.
    SetWeaponType(Option<WeaponCategory>),
    /// The flat kind picker: [`Change::SetCategory`], then
    /// [`Change::SetWeaponType`].
    SetKind(ItemKind, Option<WeaponCategory>),
    SetItem(ItemChoice),
    SetTierMode(TierMode),
    SetTier(u8),
    SetUpgradeMode(UpgradeMode),
    SetUpgrade(u8),
    SetEffectMode(EffectMode),
    /// Ticks or unticks one effect of the "Specific…" grid.
    ToggleEffect(Effect),
    SetUncursed(bool),
    SetSource(Option<ItemSource>),
    SetFloorLimitEnabled(bool),
    /// Moves the floor limit; see [`skip_boss_floor`].
    SetFloorLimit(u8),
    SetExcludeResin(bool),
    SetTransmutationsEnabled(bool),
    SetTransmutations(u8),
    SetSelectTrinket(bool),
    SetCount(u8),
    SetCopyDepthEnabled(bool),
    SetCopyDepth(u8),
    SetCountLevels(bool),
    SetTotal(u8),
    SetResinAuto(bool),
    /// The typed amount; `None` for an empty field.
    SetResinAmount(Option<f64>),
    SetIncludeMageWand(bool),
}

/// One choice of a picker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opt<T> {
    pub value: T,
    pub label: String,
    /// The heading the choice sits under (`Tier 3` in the weapon picker).
    pub group: Option<String>,
    /// A choice the picker offers only because the draft already names it —
    /// a tier-1 item or a tipped dart from an imported query — so it can be
    /// shown and saved back unchanged. Platforms may style it apart.
    pub hidden: bool,
}

impl<T> Opt<T> {
    fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            group: None,
            hidden: false,
        }
    }
}

/// A picker: its value and its choices.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice<T> {
    pub visible: bool,
    pub value: T,
    pub options: Vec<Opt<T>>,
}

/// A check box.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Toggle {
    pub visible: bool,
    pub value: bool,
    pub label: String,
    /// The help text under the check box, where it needs one.
    pub caption: Option<String>,
}

/// A mode picker with a value slider (tier, upgrade). The value is always
/// within `min..=max`, even while the mode is "any" and the slider hidden.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModeRange<M> {
    pub visible: bool,
    pub mode: M,
    pub modes: Vec<Opt<M>>,
    /// Whether the value slider shows: the control does, in a mode other
    /// than "any".
    pub value_visible: bool,
    pub value: u8,
    pub min: u8,
    pub max: u8,
    /// The value alone in words — `Tier 3 or higher`, `+2` — which a slider
    /// shows beside it; the slider keeps a fixed accessible name of the
    /// app's own.
    pub value_label: String,
}

/// One effect of the "Specific…" grid.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectChoice {
    pub value: Effect,
    pub label: String,
    pub group: EffectGroup,
    pub selected: bool,
}

/// The effect filter of a weapon or armor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectControl {
    pub visible: bool,
    /// The section's label: `Enchantment`, or `Glyph` on armor.
    pub label: String,
    pub mode: EffectMode,
    /// Any, Any enchantment (Any glyph), Specific….
    pub modes: Vec<Opt<EffectMode>>,
    /// Whether the "Specific…" grid shows: the control does, in that mode.
    pub choices_visible: bool,
    /// The family's effects in catalog order, enchantments first; curses
    /// only while the item may be cursed.
    pub choices: Vec<EffectChoice>,
    /// The grid's headings, one per group listed.
    pub groups: Vec<Opt<EffectGroup>>,
    /// What the ticked effects mean.
    pub caption: String,
}

/// A switch with a floor slider (the item's floor limit, the copies'). The
/// value is always one of `options`, which skip the empty boss floors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloorToggle {
    pub visible: bool,
    pub enabled: bool,
    pub value: u8,
    pub options: Vec<Opt<u8>>,
    pub label: String,
    /// The whole reading, not the value alone: `Within first 4 floors`,
    /// `Copies within first 4 floors`. The slider keeps a fixed accessible
    /// name of the app's own.
    pub value_label: String,
}

/// A switch with a stepper (transmutations, a combined level). The value is
/// always within `min..=max`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RangeToggle {
    pub visible: bool,
    pub enabled: bool,
    pub value: u8,
    pub min: u8,
    pub max: u8,
    pub label: String,
    /// The help text under the control.
    pub caption: Option<String>,
    /// Whether the caption shows: while the switch is on for a caption
    /// that explains the limit (transmutations), whenever the control
    /// shows for one that explains the switch (the combined level).
    pub caption_visible: bool,
    pub value_label: String,
}

/// The stack section: how many items, the copies' floor limit, and the
/// combined level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackControl {
    pub visible: bool,
    /// The section's label: `Total item count`.
    pub label: String,
    pub count: u8,
    pub min: u8,
    pub max: u8,
    /// `×2`.
    pub value_label: String,
    pub copy_depth: FloorToggle,
    pub count_levels: RangeToggle,
}

/// The Arcane Resin section.
#[derive(Clone, Debug, PartialEq)]
pub struct ResinControl {
    pub visible: bool,
    /// The section's label, `Minimum resin`, which the amount field takes
    /// too.
    pub label: String,
    pub auto: bool,
    /// The Amount/Auto choice, each valued as `auto` is: `Amount`, `Auto`.
    pub modes: Vec<Opt<bool>>,
    /// What Auto means, shown in the amount field's place while `auto` is
    /// on.
    pub caption: String,
    /// The amount as typed, shown while `auto` is off.
    pub amount: Option<f64>,
    /// The amounts that save: the query format's fixed resin minimum,
    /// [`ARCANE_RESIN_MIN`] to [`ARCANE_RESIN_MAX`].
    pub min: u16,
    pub max: u16,
    /// Counting the starting Magic Missile of a Mage run.
    pub include_mage_wand: Toggle,
}

/// Everything the sheet shows for a draft.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent facts the dialog chrome reads.
pub struct Form {
    /// [`DRAFT_VERSION`].
    pub v: u8,
    pub mode: FormMode,
    pub origin: Origin,
    pub blanket: bool,
    pub in_cluster: bool,
    pub resin_picked: bool,
    /// What the sheet is about, for its header: the requirement's title
    /// (`Any Tier 3+ melee weapon`, the item's name), or `Arcane Resin`
    /// while the resin is picked. Unlike `preview` it is there while the
    /// draft has errors, so the header never loses its subtitle; the sprite
    /// follows `item` and `kind`.
    pub title: String,
    /// The chip a save would put on the board (key 0, no join candidates),
    /// or `None` while there are errors or the sheet edits the resin.
    pub preview: Option<ChipView>,
    /// The six families.
    pub category: Choice<ItemKind>,
    /// The flat list of eight kinds, with melee and thrown weapons.
    pub kind: Choice<KindName>,
    /// Any, melee or thrown, on weapons.
    pub weapon_type: Choice<Option<WeaponCategory>>,
    pub item: Choice<ItemChoice>,
    /// Wildcard weapons and armor only.
    pub tier: ModeRange<TierMode>,
    /// Exactly +1…max, at least +1…max−1.
    pub upgrade: ModeRange<UpgradeMode>,
    pub effect: EffectControl,
    pub uncursed: Toggle,
    pub source: Choice<Option<ItemSource>>,
    pub floor_limit: FloorToggle,
    pub exclude_resin: Toggle,
    pub transmutations: RangeToggle,
    pub select_trinket: Toggle,
    pub stack: StackControl,
    pub resin: ResinControl,
    /// Why the draft cannot be saved, in the order to show them.
    pub errors: Vec<String>,
    pub can_save: bool,
}

/// What saving the query's Arcane Resin condition comes to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResinOutcome {
    /// The query's resin stays as it is: the sheet was not about resin, or
    /// saved the query's resin untouched.
    Unchanged,
    /// Arcane Resin was the picked item: the query asks for this.
    Set(ResinState),
    /// The resin chip was saved as a requirement: the query drops its resin.
    Clear,
}

/// What [`save`] did.
#[derive(Clone, Debug, PartialEq)]
// A result is built once per save and consumed at once, never stored in
// bulk, so the refused variant's size costs nothing.
#[allow(clippy::large_enum_variant)]
pub enum SaveResult {
    /// The rows after the save — a resin save removes the row the sheet was
    /// opened on — and what becomes of the query's resin.
    Saved {
        result: EditResult,
        resin: ResinOutcome,
    },
    /// The draft cannot be saved; the form says why. The draft now holds
    /// the rows the save was tried on.
    Refused { draft: Draft, form: Form },
}

/// Trinkets and artifacts: always one named find, never stacked.
const fn names_one(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Trinket | ItemKind::Artifact)
}

/// Whether the item picker offers `item_id` to a fresh sheet. Tier-1 gear is
/// starting equipment the generator never places; every shop stocks tipped
/// darts and any dart can be tipped by hand; the catalyst is the offer, not
/// a trinket one keeps.
fn pickable(item_id: ItemId) -> bool {
    item(item_id).tier != Some(1) && !item_id.is_tipped_dart() && item_id != ItemId::TrinketCatalyst
}

/// Whether `item_id` is an item of `kind` (and of the melee/thrown
/// narrowing, on weapons).
fn belongs(item_id: ItemId, kind: ItemKind, category: Option<WeaponCategory>) -> bool {
    item(item_id).kind == kind
        && (kind != ItemKind::Weapon
            || category.is_none_or(|category| item_id.weapon_category() == Some(category)))
}

/// The item a trinket or artifact sheet starts on: the first the picker
/// offers.
fn first_item(kind: ItemKind) -> Option<ItemId> {
    ITEMS
        .iter()
        .find(|definition| definition.kind == kind && pickable(definition.id))
        .map(|definition| definition.id)
}

/// A family's effects in catalog order: enchantments (or glyphs), then
/// curses.
fn family_effects(kind: ItemKind) -> Vec<Effect> {
    match kind {
        ItemKind::Weapon => ALL_WEAPON_EFFECTS
            .iter()
            .copied()
            .map(Effect::Weapon)
            .collect(),
        ItemKind::Armor => ALL_ARMOR_EFFECTS
            .iter()
            .copied()
            .map(Effect::Armor)
            .collect(),
        ItemKind::Wand | ItemKind::Ring | ItemKind::Trinket | ItemKind::Artifact => Vec::new(),
    }
}

/// A floor limit as the floor sliders hold it: within the dungeon, and off
/// the empty boss floors.
fn floor_value(depth: u8) -> u8 {
    normalize_floor_limit(depth.clamp(1, MAX_SEARCH_DEPTH))
}

/// The most transmutations a family's requirement may allow.
const fn transmutation_max(kind: ItemKind) -> u8 {
    match kind {
        ItemKind::Artifact => ARTIFACT_TRANSMUTATIONS,
        _ => TRINKET_TRANSMUTATIONS,
    }
}

/// The transmutation limit the requirement carries, whichever field holds
/// it.
const fn transmutations_of(requirement: &Requirement) -> u8 {
    if requirement.trinket_transmutations > 0 {
        requirement.trinket_transmutations
    } else {
        requirement.artifact_transmutations
    }
}

/// Sets the family's transmutation limit (0 for none).
const fn set_transmutations(requirement: &mut Requirement, value: u8) {
    match requirement.kind {
        ItemKind::Trinket => requirement.trinket_transmutations = value,
        ItemKind::Artifact => requirement.artifact_transmutations = value,
        _ => {}
    }
}

/// Pulls the upgrade under what the requirement's item and tier can reach,
/// and above +0: naming an item or narrowing the tier can put a +5 out of
/// reach, since only a tier-4 weapon is ever levelled that far, and the
/// sliders start at +1. Trinkets are never levelled, so theirs is always
/// any. An artifact's is kept: no sheet offers one, but a query may ask for
/// the +5 the city vault transfers into its artifact, and a save must not
/// drop what the sheet does not show.
fn clamp_upgrade(requirement: &mut Requirement) {
    if requirement.kind == ItemKind::Trinket {
        requirement.upgrade = UpgradeRequirement::Any;
        return;
    }
    let ceiling = requirement.upgrade_ceiling().max(1);
    requirement.upgrade = match requirement.upgrade {
        UpgradeRequirement::Any => UpgradeRequirement::Any,
        UpgradeRequirement::Exact(upgrade) => UpgradeRequirement::Exact(upgrade.clamp(1, ceiling)),
        UpgradeRequirement::AtLeast(upgrade) => {
            UpgradeRequirement::AtLeast(upgrade.clamp(1, ceiling.saturating_sub(1).max(1)))
        }
    };
}

/// A stored requirement as the sheet edits it (web
/// `namedItemEditorRequirement`, plus the slider rules): trinkets and
/// artifacts always name one, trinkets drop the tier and effect they cannot
/// carry, an at-least bound the sliders cannot show opens as what it means
/// — "+0 or higher" is any upgrade, "+max or higher" is exactly +max — and
/// a floor limit on an empty boss floor opens as the floor below, which the
/// engine reads it as anyway.
///
/// Every other field is kept, shown or not: a trinket's source, floor limit
/// and uncursed filter, an artifact's upgrade. The sheet has no control for
/// them, but the engine searches them, so a save that dropped them would
/// quietly change a query the user only meant to adjust.
///
/// What the row's family or section cannot carry goes, as a category switch
/// drops it: a hand-written list's tier on a named item, effect off weapons
/// and armor, resin exclusion off an ordinary wand, trinket selection off an
/// ordinary trinket, another family's transmutations or melee/thrown
/// narrowing. The sheet hides those controls, so it could neither show such
/// a field nor clear it, and every save would be refused for it.
fn editable(mut requirement: Requirement) -> Requirement {
    requirement.identity_group = None;
    requirement.level_sum = None;
    requirement.max_depth = requirement.max_depth.map(floor_value);
    let family = requirement.kind;
    if names_one(family) {
        requirement.item = requirement.item.or_else(|| first_item(family));
    }
    if requirement.item.is_some() || !matches!(family, ItemKind::Weapon | ItemKind::Armor) {
        requirement.tier = TierRequirement::Any;
    }
    if let EffectRequirement::OneOf(set) = requirement.effect
        && set.family() != family
    {
        requirement.effect = EffectRequirement::Any;
    }
    if family != ItemKind::Weapon {
        requirement.weapon_category = None;
    }
    if family != ItemKind::Wand || requirement.blanket {
        requirement.exclude_resin = false;
    }
    if family != ItemKind::Trinket || requirement.blanket {
        requirement.select_trinket = false;
    }
    if family != ItemKind::Trinket {
        requirement.trinket_transmutations = 0;
    }
    if family != ItemKind::Artifact {
        requirement.artifact_transmutations = 0;
    }
    let ceiling = requirement.upgrade_ceiling();
    requirement.upgrade = match requirement.upgrade {
        UpgradeRequirement::AtLeast(0) => UpgradeRequirement::Any,
        UpgradeRequirement::AtLeast(upgrade) if upgrade >= ceiling => {
            UpgradeRequirement::Exact(ceiling)
        }
        upgrade => upgrade,
    };
    clamp_upgrade(&mut requirement);
    requirement
}

/// The effect mode a requirement shows; `previous` settles the one state
/// the requirement cannot tell apart — "Specific…" with nothing ticked.
fn effect_mode_of(requirement: &Requirement, previous: EffectMode) -> EffectMode {
    match requirement.effect {
        EffectRequirement::Any if previous == EffectMode::Specific => EffectMode::Specific,
        EffectRequirement::Any => EffectMode::Any,
        EffectRequirement::OneOf(set) if EffectSet::enchantments(requirement.kind) == Some(set) => {
            EffectMode::AnyEnchantment
        }
        EffectRequirement::OneOf(_) => EffectMode::Specific,
    }
}

/// The effect filter for a selection: none ticked is no filter.
fn effect_of(effects: impl IntoIterator<Item = Effect>) -> EffectRequirement {
    EffectSet::from_effects(effects).map_or(EffectRequirement::Any, EffectRequirement::OneOf)
}

/// The trinkets the other ordinary rows name.
fn taken_trinkets(rows: &[Row], key: Option<u64>) -> Vec<ItemId> {
    let mut taken: Vec<ItemId> = Vec::new();
    for row in rows {
        let requirement = &row.requirement;
        if Some(row.key) != key
            && !requirement.blanket
            && requirement.kind == ItemKind::Trinket
            && let Some(item_id) = requirement.item
            && !taken.contains(&item_id)
        {
            taken.push(item_id);
        }
    }
    taken
}

/// The stack of the chip a key opens. A hidden copy — which no gesture can
/// name, but a platform might — opens the chip whose copy it is.
fn opened_row(rows: &[Row], key: u64) -> Option<ChipStack> {
    let index = rows.iter().position(|row| row.key == key)?;
    board_items(rows)
        .into_iter()
        .flat_map(|entry| entry.stacks)
        .find(|stack| stack.index == index || stack.copies.contains(&index))
}

/// Opens the sheet.
///
/// - With `open_resin`, on the query's Arcane Resin condition: the resin is
///   the picked item, over an any-wand draft it can be turned into. A query
///   without resin (`resin` is `None`) has no resin chip to edit, so the
///   sheet is a new one with Arcane Resin picked ([`Origin::New`]).
/// - With the key of a row on the board, on that row: its requirement, its
///   entry's stack (count, combined level, copies' floor) and whether it is
///   a cluster member come from the board. `blanket` is the row's own.
/// - Otherwise on a new chip: any weapon, or — for a blanket — the kind of
///   the first ordinary row, melee/thrown narrowing included, since a
///   blanket constrains the items the ordinary rows reserve. A key not in
///   the list is kept for the new row (Linux claims keys up front).
///
/// The resin section starts from `resin`, the query's current condition,
/// or amount 2 with uncursed donors; it is kept however the item changes.
/// `offer_resin` lets the item picker offer Arcane Resin among the wands of
/// an ordinary sheet.
#[must_use]
pub fn open(
    rows: &[Row],
    key: Option<u64>,
    blanket: bool,
    resin: Option<&ResinState>,
    offer_resin: bool,
    open_resin: bool,
) -> Draft {
    let mut draft = Draft {
        v: DRAFT_VERSION,
        origin: Origin::New,
        key,
        requirement: Requirement::any(ItemKind::Weapon),
        tier_value: DEFAULT_TIER,
        upgrade_value: DEFAULT_UPGRADE,
        effect_mode: EffectMode::Any,
        count: 1,
        total: None,
        copy_depth: None,
        floor_limit_memory: DEFAULT_FLOOR,
        copy_depth_memory: DEFAULT_FLOOR,
        transmutations_memory: 1,
        in_cluster: false,
        blanket,
        offer_resin: offer_resin || open_resin,
        taken_trinkets: Vec::new(),
        resin_picked: open_resin,
        resin: ResinDraft::seeded(resin),
        query_resin: resin.copied(),
        rows: rows.to_vec(),
    };
    if open_resin {
        // Without a resin condition there is no chip to edit: the sheet adds
        // one, and saving it as a wand clears nothing.
        draft.origin = if resin.is_some() {
            Origin::Resin
        } else {
            Origin::New
        };
        draft.key = None;
        draft.blanket = false;
        draft.requirement = Requirement::any(ItemKind::Wand);
    } else if let Some(chip) = key.and_then(|key| opened_row(rows, key)) {
        let row = rows[chip.index];
        let stack = stack_view(rows, &chip);
        draft.origin = Origin::Row(row.key);
        draft.key = Some(row.key);
        draft.blanket = row.requirement.blanket;
        draft.in_cluster = chip.in_cluster;
        draft.requirement = editable(row.requirement);
        draft.count = stack.count.clamp(1, STACK_MAX);
        // The sheet shows — and so saves — what its controls can hold: a
        // total the stack cannot reach (a shrunk stack keeps its old one)
        // comes down to what it can, a copy floor on an empty boss floor
        // to the floor below.
        let most = capacity(&draft.requirement, draft.count);
        draft.total = stack.total.map(|total| total.clamp(1, most));
        draft.copy_depth = stack.copy_depth.map(floor_value);
    } else {
        // The board shows every row, so a key in the list always opens
        // above. Should one ever not, it has no chip to edit, and a save must
        // not vanish into it: the sheet adds a new chip.
        if key.is_some_and(|key| rows.iter().any(|row| row.key == key)) {
            draft.key = None;
        }
        let template = rows
            .iter()
            .map(|row| row.requirement)
            .find(|requirement| !requirement.blanket)
            .filter(|_| blanket);
        let (kind, category) = template.map_or((ItemKind::Weapon, None), |template| {
            let category = template
                .weapon_category
                .filter(|_| template.kind == ItemKind::Weapon);
            (template.kind, category)
        });
        draft.requirement = editable(Requirement {
            weapon_category: category,
            blanket,
            ..Requirement::any(kind)
        });
    }
    remember(&mut draft);
    draft.taken_trinkets = taken_trinkets(rows, draft.key);
    draft
}

/// Fills the values the controls remember from the requirement and stack
/// the draft opened on.
fn remember(draft: &mut Draft) {
    let requirement = draft.requirement;
    draft.tier_value = match requirement.tier {
        TierRequirement::Any => DEFAULT_TIER,
        tier @ (TierRequirement::Exact(value)
        | TierRequirement::AtLeast(value)
        | TierRequirement::AtMost(value)) => {
            let (low, high) = TierMode::of(tier).range();
            value.clamp(low, high)
        }
    };
    draft.upgrade_value = match requirement.upgrade {
        UpgradeRequirement::Any => DEFAULT_UPGRADE,
        UpgradeRequirement::Exact(value) | UpgradeRequirement::AtLeast(value) => value,
    };
    draft.effect_mode = effect_mode_of(&requirement, EffectMode::Any);
    let depth = if draft.resin_picked {
        draft.resin.max_depth
    } else {
        requirement.max_depth
    };
    draft.floor_limit_memory = depth.map_or(DEFAULT_FLOOR, floor_value);
    draft.copy_depth_memory = draft.copy_depth.map_or(DEFAULT_FLOOR, floor_value);
    let transmutations = transmutations_of(&requirement);
    if transmutations > 0 {
        draft.transmutations_memory = transmutations.min(transmutation_max(requirement.kind));
    }
}

/// Whether the item picker offers Arcane Resin: an ordinary wand sheet on a
/// platform that lets the sheet set the query's resin.
fn resin_offered(draft: &Draft) -> bool {
    draft.offer_resin && draft.requirement.kind == ItemKind::Wand && !draft.blanket
}

/// Whether the sheet edits the resin rather than the wand draft.
fn picks_resin(draft: &Draft) -> bool {
    draft.resin_picked && resin_offered(draft)
}

/// The highest combined level `count` any-upgrade copies of the requirement
/// can reach in one world — what the total stepper runs to, so its range
/// does not move when counting is turned on ([`level_capacity`]).
fn capacity(requirement: &Requirement, count: u8) -> u8 {
    let copy = Requirement {
        upgrade: UpgradeRequirement::Any,
        identity_group: None,
        alternative_group: None,
        level_sum: None,
        ..*requirement
    };
    let rows = vec![copy; usize::from(count.clamp(1, STACK_MAX))];
    let stack = ChipStack {
        index: 0,
        in_cluster: false,
        copies: (1..rows.len()).collect(),
        total: None,
    };
    level_capacity(&rows, &stack).max(1)
}

/// Which controls a draft shows. [`change`] ignores the others and
/// [`form`] hides them, from this one answer, so the two never disagree.
#[allow(clippy::struct_excessive_bools)] // One flag per control.
struct Shown {
    resin: bool,
    weapon_type: bool,
    tier: bool,
    upgrade: bool,
    effect: bool,
    /// Uncursed, source and floor limit: everything but trinkets.
    details: bool,
    exclude_resin: bool,
    transmutations: bool,
    select_trinket: bool,
    stack: bool,
    count_levels: bool,
    /// The combined level is on: the stack saves a total.
    counting: bool,
    copy_depth: bool,
}

impl Shown {
    fn of(draft: &Draft) -> Self {
        let requirement = &draft.requirement;
        let family = requirement.kind;
        let resin = picks_resin(draft);
        let stack = !resin && !draft.blanket && !names_one(family);
        let count = draft.count.clamp(1, STACK_MAX);
        // A combined level is a property of a concrete lone stack of two or
        // more — of rings only, whose effects scale with their level, and
        // never of a cluster member, whose stack is its own.
        let count_levels = stack
            && !draft.in_cluster
            && requirement.item.is_some()
            && count > 1
            && family == ItemKind::Ring;
        let counting = count_levels && draft.total.is_some();
        Self {
            resin,
            weapon_type: !resin && family == ItemKind::Weapon,
            tier: !resin
                && requirement.item.is_none()
                && matches!(family, ItemKind::Weapon | ItemKind::Armor),
            // The total speaks for a counting stack's upgrades.
            upgrade: !resin && !counting && !names_one(family),
            effect: !resin && matches!(family, ItemKind::Weapon | ItemKind::Armor),
            details: family != ItemKind::Trinket,
            exclude_resin: !resin && !draft.blanket && family == ItemKind::Wand,
            transmutations: names_one(family),
            select_trinket: family == ItemKind::Trinket
                && !draft.blanket
                && requirement.trinket_transmutations == 0,
            stack,
            count_levels,
            counting,
            copy_depth: stack && count > 1 && !counting,
        }
    }
}

/// Applies one change to the draft (web `RequirementEditor`), then pulls
/// the upgrade under its ceiling. A change to a hidden control, or to a
/// value the control cannot hold (an item of another kind), returns the
/// draft unchanged.
#[must_use]
#[allow(clippy::too_many_lines)] // One arm per control.
pub fn change(draft: &Draft, change: &Change) -> Draft {
    let mut next = draft.clone();
    let shown = Shown::of(draft);
    match *change {
        Change::SetCategory(kind) => set_category(&mut next, kind),
        Change::SetWeaponType(category) => set_weapon_type(&mut next, category),
        Change::SetKind(kind, category) => {
            set_category(&mut next, kind);
            set_weapon_type(&mut next, category);
        }
        Change::SetItem(choice) => set_item(&mut next, choice),
        Change::SetTierMode(mode) if shown.tier => {
            let (low, high) = mode.range();
            if mode != TierMode::Any {
                next.tier_value = next.tier_value.clamp(low, high);
            }
            next.requirement.tier = mode.filter(next.tier_value);
        }
        Change::SetTier(tier) if shown.tier => {
            let mode = TierMode::of(next.requirement.tier);
            if mode != TierMode::Any {
                let (low, high) = mode.range();
                next.tier_value = tier.clamp(low, high);
                next.requirement.tier = mode.filter(next.tier_value);
            }
        }
        Change::SetUpgradeMode(mode) if shown.upgrade => {
            next.requirement.upgrade = mode.filter(next.upgrade_value);
        }
        Change::SetUpgrade(upgrade) if shown.upgrade => {
            let mode = UpgradeMode::of(next.requirement.upgrade);
            if mode != UpgradeMode::Any {
                next.requirement.upgrade = mode.filter(upgrade);
            }
        }
        Change::SetEffectMode(mode) if shown.effect => set_effect_mode(&mut next, mode),
        Change::ToggleEffect(effect) if shown.effect => toggle_effect(&mut next, effect),
        Change::SetUncursed(uncursed) if shown.details => set_uncursed(&mut next, uncursed),
        Change::SetSource(source) if shown.details => {
            if shown.resin {
                next.resin.source = source;
            } else {
                next.requirement.source = source;
            }
        }
        Change::SetFloorLimitEnabled(enabled) if shown.details => {
            let slot = if shown.resin {
                &mut next.resin.max_depth
            } else {
                &mut next.requirement.max_depth
            };
            switch_floor(slot, &mut next.floor_limit_memory, enabled);
        }
        Change::SetFloorLimit(depth) if shown.details => {
            let slot = if shown.resin {
                &mut next.resin.max_depth
            } else {
                &mut next.requirement.max_depth
            };
            move_floor(slot, &mut next.floor_limit_memory, depth);
        }
        Change::SetExcludeResin(exclude) if shown.exclude_resin => {
            next.requirement.exclude_resin = exclude;
        }
        Change::SetTransmutationsEnabled(enabled) if shown.transmutations => {
            switch_transmutations(&mut next, enabled);
        }
        Change::SetTransmutations(count) if shown.transmutations => {
            if transmutations_of(&next.requirement) > 0 {
                let value = count.clamp(1, transmutation_max(next.requirement.kind));
                set_transmutations(&mut next.requirement, value);
                next.transmutations_memory = value;
            }
        }
        Change::SetSelectTrinket(select) if shown.select_trinket => {
            next.requirement.select_trinket = select;
        }
        Change::SetCount(count) if shown.stack => {
            next.count = count.clamp(1, STACK_MAX);
            next.total = if next.count < 2 {
                None
            } else {
                let most = capacity(&next.requirement, next.count);
                next.total.map(|total| total.clamp(1, most))
            };
        }
        Change::SetCopyDepthEnabled(enabled) if shown.copy_depth => {
            switch_floor(&mut next.copy_depth, &mut next.copy_depth_memory, enabled);
        }
        Change::SetCopyDepth(depth) if shown.copy_depth => {
            move_floor(&mut next.copy_depth, &mut next.copy_depth_memory, depth);
        }
        Change::SetCountLevels(enabled) if shown.count_levels => {
            let start = default_total(&next.requirement, next.count);
            next.total = enabled.then(|| next.total.unwrap_or(start));
        }
        Change::SetTotal(total) if shown.counting => {
            next.total = Some(total.clamp(1, capacity(&next.requirement, next.count)));
        }
        Change::SetResinAuto(auto) if shown.resin => next.resin.auto = auto,
        Change::SetResinAmount(amount) if shown.resin && !next.resin.auto => {
            next.resin.amount = amount;
        }
        Change::SetIncludeMageWand(include) if shown.resin => {
            next.resin.include_mage_wand = include;
        }
        _ => {}
    }
    clamp_upgrade(&mut next.requirement);
    if let UpgradeRequirement::Exact(value) | UpgradeRequirement::AtLeast(value) =
        next.requirement.upgrade
    {
        next.upgrade_value = value;
    }
    next
}

/// The total a stack starts counting at: one level per item, within the
/// capacity.
fn default_total(requirement: &Requirement, count: u8) -> u8 {
    count.clamp(1, capacity(requirement, count))
}

/// Web `setKind`. Re-picking the family already chosen must not widen a
/// narrowed weapon kind or wipe the item, tier and effect, so it changes
/// nothing; another family keeps only what carries over — the source and
/// floor limit (except onto trinkets, which have neither) and the uncursed
/// filter — and starts from the wildcard, or from the first trinket or
/// artifact, which have no wildcard and never stack.
fn set_category(draft: &mut Draft, kind: ItemKind) {
    let requirement = &mut draft.requirement;
    let previous = requirement.kind;
    if kind == previous {
        return;
    }
    requirement.kind = kind;
    requirement.weapon_category = None;
    requirement.item = if names_one(kind) {
        first_item(kind)
    } else {
        None
    };
    requirement.tier = TierRequirement::Any;
    draft.tier_value = DEFAULT_TIER;
    requirement.effect = EffectRequirement::Any;
    draft.effect_mode = EffectMode::Any;
    if names_one(kind) {
        requirement.upgrade = UpgradeRequirement::Any;
    } else if names_one(previous) {
        requirement.upgrade = UpgradeRequirement::Any;
        draft.upgrade_value = DEFAULT_UPGRADE;
    }
    if kind == ItemKind::Trinket {
        requirement.require_uncursed = false;
        requirement.source = None;
        requirement.max_depth = None;
    } else {
        requirement.select_trinket = false;
        requirement.trinket_transmutations = 0;
    }
    if kind != ItemKind::Artifact {
        requirement.artifact_transmutations = 0;
    }
    if kind != ItemKind::Wand {
        requirement.exclude_resin = false;
    }
    if names_one(kind) {
        draft.count = 1;
        draft.total = None;
        draft.copy_depth = None;
    }
    // Arcane Resin is a wand-sheet choice; the new family starts on its own
    // item picker.
    draft.resin_picked = false;
}

/// Any, melee or thrown, on a weapon draft; the item stays only if it is of
/// that type.
fn set_weapon_type(draft: &mut Draft, category: Option<WeaponCategory>) {
    let requirement = &mut draft.requirement;
    if requirement.kind != ItemKind::Weapon {
        return;
    }
    requirement.weapon_category = category;
    if requirement
        .item
        .is_some_and(|item_id| !belongs(item_id, ItemKind::Weapon, category))
    {
        requirement.item = None;
    }
}

/// The item picker (web): the wildcard drops the combined level, which
/// only a named ring counts; naming an item drops the tier filter, since a
/// named item is the tier it is; Arcane Resin switches the sheet to the
/// resin section and back without touching the wand draft.
fn set_item(draft: &mut Draft, choice: ItemChoice) {
    let requirement = &mut draft.requirement;
    match choice {
        ItemChoice::Any if !names_one(requirement.kind) => {
            requirement.item = None;
            draft.total = None;
            draft.resin_picked = false;
        }
        ItemChoice::Item(item_id)
            if belongs(item_id, requirement.kind, requirement.weapon_category) =>
        {
            requirement.item = Some(item_id);
            requirement.tier = TierRequirement::Any;
            draft.resin_picked = false;
        }
        ItemChoice::ArcaneResin if resin_offered(draft) => draft.resin_picked = true,
        _ => {}
    }
}

/// The effect mode: "any" clears the filter, "any enchantment" takes the
/// family's whole non-curse set, and "Specific…" keeps a set being built —
/// but starts empty from "any enchantment", whose set was not ticked by hand.
fn set_effect_mode(draft: &mut Draft, mode: EffectMode) {
    let requirement = &mut draft.requirement;
    match mode {
        EffectMode::Any => requirement.effect = EffectRequirement::Any,
        EffectMode::AnyEnchantment => {
            requirement.effect = EffectSet::enchantments(requirement.kind)
                .map_or(EffectRequirement::Any, EffectRequirement::OneOf);
        }
        EffectMode::Specific => {
            if effect_mode_of(requirement, draft.effect_mode) == EffectMode::AnyEnchantment {
                requirement.effect = EffectRequirement::Any;
            }
        }
    }
    draft.effect_mode = mode;
}

/// Ticks or unticks one effect in "Specific…". Ticking the whole non-curse
/// set is "any enchantment", and the mode follows.
fn toggle_effect(draft: &mut Draft, effect: Effect) {
    let requirement = &mut draft.requirement;
    let family = requirement.kind;
    if effect_mode_of(requirement, draft.effect_mode) != EffectMode::Specific
        || !family_effects(family).contains(&effect)
        || (requirement.require_uncursed && effect.is_curse())
    {
        return;
    }
    let mut chosen: Vec<Effect> = match requirement.effect {
        EffectRequirement::OneOf(set) if set.family() == family => set.effects().collect(),
        _ => Vec::new(),
    };
    if let Some(position) = chosen.iter().position(|&known| known == effect) {
        chosen.remove(position);
    } else {
        chosen.push(effect);
    }
    requirement.effect = effect_of(chosen);
    draft.effect_mode = effect_mode_of(requirement, EffectMode::Specific);
}

/// The uncursed filter. Curses leave the selection as they leave the grid:
/// an uncursed item can carry none.
fn set_uncursed(draft: &mut Draft, uncursed: bool) {
    if picks_resin(draft) {
        draft.resin.uncursed = uncursed;
        return;
    }
    let requirement = &mut draft.requirement;
    requirement.require_uncursed = uncursed;
    if uncursed && let EffectRequirement::OneOf(set) = requirement.effect {
        requirement.effect = effect_of(set.effects().filter(|effect| !effect.is_curse()));
        draft.effect_mode = effect_mode_of(requirement, draft.effect_mode);
    }
}

/// A floor-limit switch: on at the remembered floor, off remembering it.
fn switch_floor(slot: &mut Option<u8>, memory: &mut u8, enabled: bool) {
    if enabled {
        slot.get_or_insert(floor_value(*memory));
    } else if let Some(depth) = slot.take() {
        *memory = floor_value(depth);
    }
}

/// A floor-limit slider move, stepping over the empty boss floors
/// ([`skip_boss_floor`]). A switched-off slider does not move.
fn move_floor(slot: &mut Option<u8>, memory: &mut u8, requested: u8) {
    if let Some(previous) = *slot {
        let depth = skip_boss_floor(previous, requested.clamp(1, MAX_SEARCH_DEPTH));
        *slot = Some(depth);
        *memory = depth;
    }
}

/// The transmutation switch: on at the remembered count — which rules out
/// choosing the trinket at +3, an initial-offer rule — off remembering it.
fn switch_transmutations(draft: &mut Draft, enabled: bool) {
    let requirement = &mut draft.requirement;
    let current = transmutations_of(requirement);
    if enabled {
        if current == 0 {
            let value = draft
                .transmutations_memory
                .clamp(1, transmutation_max(requirement.kind));
            set_transmutations(requirement, value);
        }
        requirement.select_trinket = false;
    } else if current > 0 {
        draft.transmutations_memory = current;
        set_transmutations(requirement, 0);
    }
}

/// The requirement a save stores: the draft's, marked with its section.
/// Stack and combined-level labels come from the count and total; the
/// alternative group is the edited row's (see [`Edit::Save`]).
fn saved_requirement(draft: &Draft) -> Requirement {
    Requirement {
        identity_group: None,
        level_sum: None,
        blanket: draft.blanket,
        ..draft.requirement
    }
}

/// A save tried on a draft's rows: the rows it produces, the key it saved,
/// and why it may not.
struct Attempt {
    result: EditResult,
    /// The saved row's key; `None` for a resin save.
    saved: Option<u64>,
    errors: Vec<String>,
}

/// Runs the save on `draft.rows` and gathers every error: the saved
/// requirement's own problems ([`row_problems`]), a duplicate trinket, the
/// resin amount, and the save guard.
///
/// The guard compares the list's problems before and after the save: a
/// problem that blames the saved row and did not blame it before is one the
/// save would cause — turning a cluster member of a stack into another
/// category, say, which every platform but Linux used to store. Problems
/// the list already had, or that blame other rows only (an unrelated invalid
/// row, a first blanket before any ordinary row), never block the save.
///
/// The rows may have changed since the sheet opened. When the draft's row
/// has since become a hidden copy of another chip, the save adds a new chip
/// instead — as [`open`] does for a key the board shows nowhere — rather
/// than vanishing into the copy.
fn attempt(draft: &Draft, hint: Option<u64>, held: &HeldLabels) -> Attempt {
    let (rows, rekeyed) = repair_keys(&draft.rows, hint);
    // Keys the repair gave up (zero, out of range) follow their row.
    let resolve = |key: u64| {
        rekeyed
            .iter()
            .find(|&&(old, _)| old == key && !is_valid_key(old))
            .map_or(key, |&(_, new)| new)
    };
    let adopt = |mut result: EditResult| {
        if !rekeyed.is_empty() {
            result.changed = true;
            result.rekeyed.clone_from(&rekeyed);
        }
        result
    };
    let mut errors: Vec<String> = Vec::new();
    if picks_resin(draft) {
        if draft.resin.state().is_none() {
            errors.push(RESIN_AMOUNT_RANGE.to_owned());
        }
        // A wand chip turned into the resin leaves the board.
        let edits: Vec<Edit> = match draft.origin {
            Origin::Row(key) => vec![Edit::Remove { key: resolve(key) }],
            Origin::New | Origin::Resin => Vec::new(),
        };
        return Attempt {
            result: adopt(apply_holding(&rows, hint, &edits, held)),
            saved: None,
            errors,
        };
    }
    let requirement = saved_requirement(draft);
    errors.extend(row_problems(&Requirement {
        alternative_group: None,
        ..requirement
    }));
    if !requirement.blanket
        && requirement.kind == ItemKind::Trinket
        && requirement
            .item
            .is_some_and(|item_id| draft.taken_trinkets.contains(&item_id))
    {
        errors.push(DUPLICATE_TRINKET.to_owned());
    }
    let key = draft
        .key
        .map(resolve)
        .filter(|&key| is_valid_key(key) && !folded_away(&rows, key))
        .unwrap_or_else(|| mint_key(&rows, hint));
    let before = problems(&rows);
    let saved = apply_holding(&rows, hint, &[save_edit(draft, key)], held);
    let result = if stores_nothing(draft, &rows, key)
        && (saved.refused.is_some() || !repairs(&before, &problems(&saved.rows)))
    {
        // Nothing to write and nothing to repair; the sheet still returns to
        // its chip.
        let mut result = apply_holding(&rows, hint, &[], held);
        result.focus = chip_of(&rows, key);
        result
    } else {
        saved
    };
    if let Some(refusal) = result.refused {
        errors.push(refusal.to_string());
    } else {
        for problem in problems(&result.rows) {
            let caused = problem.keys.contains(&key)
                && !before
                    .iter()
                    .any(|old| old.message == problem.message && old.keys.contains(&key));
            if caused && !errors.contains(&problem.message) {
                errors.push(problem.message);
            }
        }
    }
    Attempt {
        result: adopt(result),
        saved: Some(key),
        errors,
    }
}

/// The board edit a save of `draft` writes onto the row `key`: its
/// requirement, with a count only where the sheet shows one, a total only
/// while the combined level is on, and a copy floor only while it shows —
/// or while the combined level, which hides it, keeps it.
fn save_edit(draft: &Draft, key: u64) -> Edit {
    let shown = Shown::of(draft);
    Edit::Save {
        key: Some(key),
        requirement: saved_requirement(draft),
        count: if shown.stack { draft.count } else { 1 },
        total: draft.total.filter(|_| shown.counting),
        copy_depth: draft
            .copy_depth
            .filter(|_| shown.copy_depth || shown.counting),
    }
}

/// Whether saving `draft`, opened on a row, onto the row `key` of `rows`
/// stores what the row already says: the sheet opened on it now would save
/// the very same edit. Such a save writes nothing, so the row keeps what the
/// sheet's controls cannot hold — a "+0 or higher", a floor limit on an
/// empty boss floor, a combined level beyond reach, copies with floors of
/// their own — and a list no one changed stays the list a platform stored
/// (Android's refine plan and the web's preset match compare it whole).
///
/// A row a hand-written list got wrong is the exception: where the save
/// [`repairs`] the list — a floor beyond the dungeon, a field the row's
/// family or section cannot carry — it writes, as a save that changed
/// something would.
fn stores_nothing(draft: &Draft, rows: &[Row], key: u64) -> bool {
    matches!(draft.origin, Origin::Row(_))
        && rows.iter().any(|row| row.key == key)
        && save_edit(
            &open(rows, Some(key), draft.blanket, None, false, false),
            key,
        ) == save_edit(draft, key)
}

/// Whether a list with the problems `after` is a repair of one with the
/// problems `before`: one of them is gone and none is new. A problem is the
/// same whatever order its keys come in, since a save may move a stack's
/// copies.
fn repairs(before: &[Problem], after: &[Problem]) -> bool {
    let same = |one: &Problem, other: &Problem| {
        let sorted = |keys: &[u64]| {
            let mut keys = keys.to_vec();
            keys.sort_unstable();
            keys
        };
        one.message == other.message
            && one.scope == other.scope
            && sorted(&one.keys) == sorted(&other.keys)
    };
    let within = |problem: &Problem, list: &[Problem]| list.iter().any(|old| same(old, problem));
    after.iter().all(|problem| within(problem, before))
        && before.iter().any(|problem| !within(problem, after))
}

/// The chip showing the row `key`: the row itself, or the chip whose copy
/// it is.
fn chip_of(rows: &[Row], key: u64) -> Option<u64> {
    opened_row(rows, key).map(|stack| rows[stack.index].key)
}

/// Whether the row with `key` is in `rows` as a hidden copy — no board
/// entry's member — which has no sheet of its own.
fn folded_away(rows: &[Row], key: u64) -> bool {
    rows.iter()
        .position(|row| row.key == key)
        .is_some_and(|index| {
            !board_items(rows)
                .iter()
                .any(|entry| entry.members.contains(&index))
        })
}

/// The chip the attempt put on the board, as the sheet previews it: the
/// saved row's own chip, or — when it folded into an earlier chip as a
/// plain repeat — that chip, with its stack and badges. It is not on the
/// board yet, so it has no key, no copy keys (so no remaining badges) and
/// no join candidates.
fn preview(attempt: &Attempt) -> Option<ChipView> {
    let saved = attempt.saved?;
    if !attempt.errors.is_empty() {
        return None;
    }
    let board = board_view(&attempt.result.rows, None);
    let entry = board
        .items
        .into_iter()
        .find(|entry| entry.members.contains(&saved) || entry.extras.contains(&saved))?;
    let mut chip = entry
        .chips
        .iter()
        .find(|chip| chip.key == saved)
        .or_else(|| entry.chips.first())?
        .clone();
    chip.key = 0;
    chip.copies.clear();
    chip.remaining_badges = None;
    chip.lifted = None;
    chip.join.clear();
    chip.refuse.clear();
    Some(chip)
}

/// What the sheet shows for `draft`: every control, the preview chip and
/// the errors. The save guard runs the save on the rows the sheet was
/// opened on.
#[must_use]
pub fn form(draft: &Draft) -> Form {
    form_of(draft, &attempt(draft, None, &HeldLabels::default()))
}

/// Saves the draft onto `rows`, the list as it is now (the sheet may have
/// been open while it changed): the requirement as [`Edit::Save`] with its
/// stack — a count only where the sheet shows one, a total only while the
/// combined level is on, a copy floor only while it shows — or, with
/// Arcane Resin picked, the query's resin condition, removing the wand chip
/// the sheet was opened on. Saving the resin chip as a requirement clears
/// the query's resin. A sheet saved untouched writes nothing: the rows —
/// or the query's resin — come back as they were, even where they hold what
/// no control can show, unless the save repairs a problem the row had. A
/// draft with errors is refused with its form.
#[must_use]
pub fn save(draft: &Draft, rows: &[Row], next_key: Option<u64>) -> SaveResult {
    save_holding(draft, rows, next_key, &HeldLabels::default())
}

/// [`save`] onto a list beside rows the editor cannot read, whose group
/// labels the save may not take (see [`HeldLabels`]).
pub(crate) fn save_holding(
    draft: &Draft,
    rows: &[Row],
    next_key: Option<u64>,
    held: &HeldLabels,
) -> SaveResult {
    let draft = Draft {
        taken_trinkets: taken_trinkets(rows, draft.key),
        rows: rows.to_vec(),
        ..draft.clone()
    };
    let attempt = attempt(&draft, next_key, held);
    if !attempt.errors.is_empty() {
        let form = form_of(&draft, &attempt);
        return SaveResult::Refused { draft, form };
    }
    let resin = if picks_resin(&draft) {
        match draft.resin.state() {
            Some(state) if !keeps_query_resin(&draft, &state) => ResinOutcome::Set(state),
            _ => ResinOutcome::Unchanged,
        }
    } else if draft.origin == Origin::Resin {
        ResinOutcome::Clear
    } else {
        ResinOutcome::Unchanged
    };
    SaveResult::Saved {
        result: attempt.result,
        resin,
    }
}

/// Whether the resin chip's sheet saves the query's resin untouched: it
/// would store `state`, which is what the sheet opened on the query's resin
/// would store. Such a save leaves the resin as it is, so it keeps what the
/// floor slider cannot hold — a limit on an empty boss floor, which the
/// engine reads as the floor below anyway. A limit beyond the dungeon, which
/// the query format refuses, is repaired instead.
fn keeps_query_resin(draft: &Draft, state: &ResinState) -> bool {
    draft.origin == Origin::Resin
        && draft.query_resin.is_some_and(|query| {
            query
                .filter
                .max_depth
                .is_none_or(|depth| (1..=MAX_SEARCH_DEPTH).contains(&depth))
                && ResinDraft::seeded(Some(&query)).state() == Some(*state)
        })
}

/// The floors a floor slider offers: 1–24 without the empty boss floors.
fn floor_options() -> Vec<Opt<u8>> {
    (1..=MAX_SEARCH_DEPTH)
        .filter(|floor| !EMPTY_BOSS_FLOORS.contains(floor))
        .map(|floor| Opt::new(floor, floor.to_string()))
        .collect()
}

/// `N floor(s)`.
fn floors(depth: u8) -> String {
    let plural = if depth == 1 { "" } else { "s" };
    format!("{depth} floor{plural}")
}

#[allow(clippy::too_many_lines)] // One field per control.
fn form_of(draft: &Draft, attempt: &Attempt) -> Form {
    let requirement = &draft.requirement;
    let family = requirement.kind;
    let shown = Shown::of(draft);
    Form {
        v: DRAFT_VERSION,
        mode: if draft.origin == Origin::New {
            FormMode::New
        } else {
            FormMode::Edit
        },
        origin: draft.origin,
        blanket: draft.blanket,
        in_cluster: draft.in_cluster,
        resin_picked: shown.resin,
        title: if shown.resin {
            ARCANE_RESIN.to_owned()
        } else {
            requirement_title(&saved_requirement(draft))
        },
        preview: preview(attempt),
        category: Choice {
            visible: true,
            value: family,
            options: [
                ItemKind::Weapon,
                ItemKind::Armor,
                ItemKind::Wand,
                ItemKind::Ring,
                ItemKind::Trinket,
                ItemKind::Artifact,
            ]
            .into_iter()
            .map(|kind| Opt::new(kind, category_label(kind)))
            .collect(),
        },
        kind: Choice {
            visible: true,
            value: KindName::of_requirement(requirement),
            options: KindName::ALL
                .into_iter()
                .map(|kind| Opt::new(kind, kind.label()))
                .collect(),
        },
        weapon_type: Choice {
            visible: shown.weapon_type,
            value: requirement.weapon_category,
            options: [
                None,
                Some(WeaponCategory::Melee),
                Some(WeaponCategory::Thrown),
            ]
            .into_iter()
            .map(|category| Opt::new(category, weapon_type_label(category)))
            .collect(),
        },
        item: Choice {
            visible: true,
            value: if shown.resin {
                ItemChoice::ArcaneResin
            } else {
                requirement.item.map_or(ItemChoice::Any, ItemChoice::Item)
            },
            options: item_options(draft),
        },
        tier: tier_control(draft, &shown),
        upgrade: upgrade_control(draft, &shown),
        effect: effect_control(draft, &shown),
        uncursed: Toggle {
            visible: shown.details,
            value: if shown.resin {
                draft.resin.uncursed
            } else {
                requirement.require_uncursed
            },
            label: if shown.resin {
                "Require uncursed wands"
            } else {
                "Require uncursed"
            }
            .to_owned(),
            caption: None,
        },
        source: Choice {
            visible: shown.details,
            value: if shown.resin {
                draft.resin.source
            } else {
                requirement.source
            },
            options: std::iter::once(Opt::new(None, "Any"))
                .chain(
                    ItemSource::ALL
                        .iter()
                        .map(|&source| Opt::new(Some(source), source.label())),
                )
                .collect(),
        },
        floor_limit: floor_toggle(
            shown.details,
            if shown.resin {
                draft.resin.max_depth
            } else {
                requirement.max_depth
            },
            draft.floor_limit_memory,
            if shown.resin {
                "Limit wands to a floor"
            } else {
                "Limit this item to a floor"
            },
            "Within first",
        ),
        exclude_resin: Toggle {
            visible: shown.exclude_resin,
            value: requirement.exclude_resin,
            label: "Exclude from Auto resin".to_owned(),
            caption: Some(
                "Keep this wand without budgeting resin to upgrade it. Useful for imbuing: resin \
                 upgrades do not transfer to the staff. Extra copies are reserved for reforging \
                 and never need Auto resin."
                    .to_owned(),
            ),
        },
        transmutations: transmutations_control(draft, &shown),
        select_trinket: Toggle {
            visible: shown.select_trinket,
            value: requirement.select_trinket,
            label: "Choose matching trinket at +3".to_owned(),
            caption: Some(
                "Applies after the first brewing opportunity. If several alternatives are \
                 offered, no trinket is chosen."
                    .to_owned(),
            ),
        },
        stack: stack_control(draft, &shown),
        resin: resin_control(draft, &shown),
        can_save: attempt.errors.is_empty(),
        errors: attempt.errors.clone(),
    }
}

/// The item picker (web): the wildcard (`Any melee weapon`) unless the
/// family always names one, Arcane Resin when offered, then the items in
/// catalog order — weapons under `Tier 2`…`Tier 5` headings. Tier-1 gear,
/// tipped darts and the catalyst are left out, except the item the draft
/// already names, which is listed where the catalog puts it and marked
/// hidden, so an imported requirement shows and saves back unchanged.
fn item_options(draft: &Draft) -> Vec<Opt<ItemChoice>> {
    let requirement = &draft.requirement;
    let (family, category) = (requirement.kind, requirement.weapon_category);
    let mut options = Vec::new();
    if !names_one(family) {
        options.push(Opt::new(ItemChoice::Any, wildcard_label(family, category)));
    }
    if resin_offered(draft) {
        options.push(Opt::new(ItemChoice::ArcaneResin, ARCANE_RESIN));
    }
    let mut listed: Vec<&ItemDefinition> = ITEMS
        .iter()
        .filter(|definition| {
            belongs(definition.id, family, category)
                && (pickable(definition.id) || requirement.item == Some(definition.id))
        })
        .collect();
    let by_tier = family == ItemKind::Weapon;
    if by_tier {
        // Stable: catalog order within each tier.
        listed.sort_by_key(|definition| definition.tier);
    }
    options.extend(listed.into_iter().map(|definition| Opt {
        value: ItemChoice::Item(definition.id),
        label: definition.display_name.to_owned(),
        group: by_tier.then(|| format!("Tier {}", definition.tier.unwrap_or_default())),
        hidden: !pickable(definition.id),
    }));
    options
}

/// The Arcane Resin section, in the words every platform's resin sheet
/// shared.
fn resin_control(draft: &Draft, shown: &Shown) -> ResinControl {
    ResinControl {
        visible: shown.resin,
        label: RESIN_MINIMUM.to_owned(),
        auto: draft.resin.auto,
        modes: vec![Opt::new(false, RESIN_AMOUNT), Opt::new(true, RESIN_AUTO)],
        caption: RESIN_AUTO_CAPTION.to_owned(),
        amount: draft.resin.amount,
        min: ARCANE_RESIN_MIN,
        max: ARCANE_RESIN_MAX,
        include_mage_wand: Toggle {
            visible: shown.resin,
            value: draft.resin.include_mage_wand,
            label: RESIN_MAGE_WAND.to_owned(),
            caption: Some(RESIN_MAGE_WAND_CAPTION.to_owned()),
        },
    }
}

fn tier_control(draft: &Draft, shown: &Shown) -> ModeRange<TierMode> {
    let mode = TierMode::of(draft.requirement.tier);
    let (min, max) = mode.range();
    let value = match draft.requirement.tier {
        TierRequirement::Any => draft.tier_value,
        TierRequirement::Exact(value)
        | TierRequirement::AtLeast(value)
        | TierRequirement::AtMost(value) => value,
    }
    .clamp(min, max);
    ModeRange {
        visible: shown.tier,
        mode,
        modes: TierMode::ALL
            .into_iter()
            .map(|mode| Opt::new(mode, mode.label()))
            .collect(),
        value_visible: shown.tier && mode != TierMode::Any,
        value,
        min,
        max,
        value_label: match mode {
            TierMode::Any | TierMode::Exact => format!("Tier {value}"),
            TierMode::AtLeast => format!("Tier {value} or higher"),
            TierMode::AtMost => format!("Tier {value} or lower"),
        },
    }
}

fn upgrade_control(draft: &Draft, shown: &Shown) -> ModeRange<UpgradeMode> {
    let mode = UpgradeMode::of(draft.requirement.upgrade);
    let ceiling = draft.requirement.upgrade_ceiling().max(1);
    // "At least +max" is "exactly +max", so the bound stops one short.
    let max = if mode == UpgradeMode::AtLeast {
        ceiling.saturating_sub(1).max(1)
    } else {
        ceiling
    };
    let value = match draft.requirement.upgrade {
        UpgradeRequirement::Any => draft.upgrade_value,
        UpgradeRequirement::Exact(value) | UpgradeRequirement::AtLeast(value) => value,
    }
    .clamp(1, max);
    ModeRange {
        visible: shown.upgrade,
        mode,
        modes: UpgradeMode::ALL
            .into_iter()
            .map(|mode| Opt::new(mode, mode.label()))
            .collect(),
        value_visible: shown.upgrade && mode != UpgradeMode::Any,
        value,
        min: 1,
        max,
        value_label: if mode == UpgradeMode::AtLeast {
            format!("+{value} or higher")
        } else {
            format!("+{value}")
        },
    }
}

fn effect_control(draft: &Draft, shown: &Shown) -> EffectControl {
    let requirement = &draft.requirement;
    let family = requirement.kind;
    let chosen = match requirement.effect {
        EffectRequirement::OneOf(set) if set.family() == family => Some(set),
        _ => None,
    };
    // Curses leave the grid while the item must be uncursed.
    let curses = !requirement.require_uncursed;
    let choices: Vec<EffectChoice> = family_effects(family)
        .into_iter()
        .filter(|effect| curses || !effect.is_curse())
        .map(|effect| EffectChoice {
            value: effect,
            label: effect.wire_name().to_owned(),
            group: if effect.is_curse() {
                EffectGroup::Curse
            } else {
                EffectGroup::Enchantment
            },
            selected: chosen.is_some_and(|set| set.contains(effect)),
        })
        .collect();
    let mut groups = Vec::new();
    if matches!(family, ItemKind::Weapon | ItemKind::Armor) {
        groups.push(Opt::new(
            EffectGroup::Enchantment,
            EffectGroup::Enchantment.label(family),
        ));
        if curses {
            groups.push(Opt::new(
                EffectGroup::Curse,
                EffectGroup::Curse.label(family),
            ));
        }
    }
    let ticked = choices.iter().filter(|choice| choice.selected).count();
    let mode = effect_mode_of(requirement, draft.effect_mode);
    EffectControl {
        visible: shown.effect,
        label: effect_section_label(family).to_owned(),
        mode,
        modes: EffectMode::ALL
            .into_iter()
            .map(|mode| Opt::new(mode, mode.label(family)))
            .collect(),
        choices_visible: shown.effect && mode == EffectMode::Specific,
        choices,
        groups,
        caption: match ticked {
            0 => "Tick the effects the item may carry; none ticked means any.".to_owned(),
            1 => "Matches any one of 1 effect.".to_owned(),
            ticked => format!("Matches any one of {ticked} effects."),
        },
    }
}

/// A floor switch and slider; `value_prefix` leads the value in words.
fn floor_toggle(
    visible: bool,
    depth: Option<u8>,
    memory: u8,
    label: &str,
    value_prefix: &str,
) -> FloorToggle {
    let value = floor_value(depth.unwrap_or(memory));
    FloorToggle {
        visible,
        enabled: depth.is_some(),
        value,
        options: floor_options(),
        label: label.to_owned(),
        value_label: format!("{value_prefix} {}", floors(value)),
    }
}

fn transmutations_control(draft: &Draft, shown: &Shown) -> RangeToggle {
    let requirement = &draft.requirement;
    let current = transmutations_of(requirement);
    let max = transmutation_max(requirement.kind);
    let value = if current > 0 {
        current
    } else {
        draft.transmutations_memory
    }
    .clamp(1, max);
    let caption = if requirement.kind == ItemKind::Artifact {
        "Includes natural finds, or transforms an obtainable artifact using the remaining deck \
         at the floor limit. Source and curse filters apply to the starting artifact. Scroll \
         availability and later generation changes are not simulated."
            .to_owned()
    } else {
        let trinkets = if value == 1 { "trinket" } else { "trinkets" };
        format!(
            "Matches an initial offer or any of the next {value} {trinkets}. AutoTrinket can use \
             a helpful starting trinket while your target waits in the deck. Scroll availability \
             and effects after transmuting are not simulated."
        )
    };
    RangeToggle {
        visible: shown.transmutations,
        enabled: current > 0,
        value,
        min: 1,
        max,
        label: "Allow transmutations".to_owned(),
        caption: Some(caption),
        caption_visible: shown.transmutations && current > 0,
        value_label: format!("At most {value}"),
    }
}

fn stack_control(draft: &Draft, shown: &Shown) -> StackControl {
    let count = draft.count.clamp(1, STACK_MAX);
    let most = capacity(&draft.requirement, count);
    let total = draft
        .total
        .unwrap_or_else(|| default_total(&draft.requirement, count))
        .clamp(1, most);
    StackControl {
        visible: shown.stack,
        label: "Total item count".to_owned(),
        count,
        min: 1,
        max: STACK_MAX,
        value_label: count_text(count, false),
        copy_depth: floor_toggle(
            shown.copy_depth,
            draft.copy_depth,
            draft.copy_depth_memory,
            "Limit the extra copies to a floor",
            "Copies within first",
        ),
        count_levels: RangeToggle {
            visible: shown.count_levels,
            enabled: shown.counting,
            value: total,
            min: 1,
            max: most,
            label: "Count levels together".to_owned(),
            caption: Some(
                "Each item counts its upgrade plus one, and spare items may go unused.".to_owned(),
            ),
            caption_visible: shown.count_levels,
            value_label: format!("≥ {total} across up to {count}"),
        },
    }
}

#[cfg(test)]
mod tests;
