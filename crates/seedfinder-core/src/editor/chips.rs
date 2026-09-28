//! The board as every frontend draws it: entries, chips, badges, the resin
//! chip, and every word on them.
//!
//! [`board_view`] is one pass over the list that answers everything a board
//! render asks — which rows collapse into which entry, what each chip is
//! called and tagged, what its popover says, which chips it may join or is
//! refused by, and what is wrong with it. The platforms keep the drawing:
//! sprites, glow colours, layout, gestures. Before, each carried its own
//! chip text and the six boards disagreed on tag order, tier wording, badge
//! spacing and which chips were marked as errors.
//!
//! The text follows the web board (`RequirementBoard.tsx`: the chip, its
//! badges and its `ChipPopover`) with the shared design's settled wording:
//! title-case item names, "Any Tier 3 weapon" titles, the short "Any
//! melee" chip names, "any glyph" on armor, and a "choose at +3" tag.

use std::collections::BTreeSet;

use crate::catalog::{Effect, ItemId, ItemKind};
use crate::query::{
    ArcaneResinFilter, EffectRequirement, EffectSet, Requirement, UpgradeRequirement,
};

use super::Row;
use super::board::{
    BoardItem, ChipStack, Edit, HeldLabels, ItemKey, JoinCandidates, Refusal, apply_holding,
    board_items, join_candidates_holding, lifted,
};
use super::labels::{
    ARCANE_RESIN, CopyFloors, EXCLUDED_FROM_RESIN, KindName, NO_RESIN, RESIN_AUTO,
    RESIN_AUTO_TOOLTIP, RESIN_MAGE_DETAIL, RESIN_MAGE_TAG, RESIN_MAGE_TOOLTIP, SELECT_TRINKET,
    UNCURSED, alternatives_label, compact_total_text, count_text, count_tooltip, effect_label,
    entry_name, floor_detail, floor_tag, level_sum_relation, requirement_name, requirement_title,
    stack_relation, tier_tag, total_text, total_tooltip, transmutations_detail, transmutations_tag,
    upgrade_detail, upgrade_tag,
};
use super::problems::{IndexedProblem, Problem, ProblemScope, Unread, indexed_problems, keyed};
use super::stack::{StackView, stack_view};

/// Everything a board render needs, from one fold of the list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoardView {
    /// The board's entries in list order, both sections together: a
    /// platform splits them by [`ItemView::blanket`].
    pub items: Vec<ItemView>,
    pub counts: Counts,
    /// Everything wrong with the list ([`super::problems()`]).
    pub problems: Vec<Problem>,
    /// The Arcane Resin chip, when the query asks for resin.
    pub resin: Option<ResinChip>,
}

/// How many entries each board section shows — what a section header
/// counts as its "requirements", clusters and stacks counting once.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counts {
    pub ordinary: usize,
    pub blanket: usize,
}

/// One board entry: a chip, or an either/or cluster of chips. Its badges
/// and steppers are its chips': nothing is drawn or counted per cluster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemView {
    /// The entry's identity, stable while it survives an edit.
    pub id: ItemKey,
    /// Which section the entry sits in (its anchor's).
    pub blanket: bool,
    /// The alternative group of a cluster of two or more.
    pub cluster: Option<u8>,
    /// A cluster's caption, `Any of N`.
    pub label: Option<String>,
    /// The entry's name where a menu or a drag caption names it — an
    /// "Either/or with…" choice: a chip's name, or a cluster's members'
    /// names joined with ` or ` (`Spear or Mace`).
    pub name: String,
    /// The visible rows' keys: one for a chip, every member of a cluster.
    pub members: Vec<u64>,
    /// Every hidden copy's key: the copies behind all its chips' badges,
    /// each once, in list order.
    pub extras: Vec<u64>,
    /// One chip per member, in member order.
    pub chips: Vec<ChipView>,
    /// The first problem touching any member or hidden copy, so a problem
    /// on a copy the board folds away still shows somewhere.
    pub problem: Option<String>,
}

/// The badges a chip shows at rest. The steppers that edit them read
/// [`StackView::count_text`] and [`StackView::total_text`] instead, which
/// exist even when the badge does not.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Badges {
    /// `×N` (or `≤N` while counting levels), when the chip asks for more
    /// than one item.
    pub count: Option<Badge>,
    /// `Σ ≥ T`, when the stack counts levels together.
    pub total: Option<Badge>,
}

/// One badge's words.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Badge {
    pub text: String,
    /// The same with no spaces, for boards where a chip gives up its name
    /// before its badges (the phones).
    pub compact_text: String,
    pub tooltip: String,
}

/// How a chip tag is drawn.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TagStyle {
    Plain,
    /// The upgrade, tinted apart from the rest.
    Upgrade,
    /// The resin the resin chip counts — its amount and the Mage's credit —
    /// tinted apart from the donor filters.
    Credit,
}

/// A qualifier beside a chip's name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub text: String,
    pub style: TagStyle,
    /// The tag's own hover text, where the tag needs explaining: what
    /// `Auto` means, where `Mage +2` comes from. `None` for the rest.
    pub tooltip: Option<String>,
}

impl Tag {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TagStyle::Plain,
            tooltip: None,
        }
    }
}

/// The effect filter as a chip shows it. Platforms pick the cue — a glow on
/// a named item's sprite, a dot, a ring of colours, a count — from these
/// facts; the colours are theirs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectBadge {
    /// The filter in words: `any enchantment` (`any glyph` on armor), one
    /// effect's name, or `effect: A/B/C`.
    pub label: String,
    /// Every accepted effect in catalog order — the full non-curse set for
    /// "any enchantment", so a glow can be drawn from it.
    pub effects: Vec<Effect>,
    /// Whether the filter is "any enchantment" (or "any glyph"), which
    /// platforms draw as its own cue rather than as a count of effects.
    pub any_enchantment: bool,
    /// Whether the filter accepts curses only.
    pub curses_only: bool,
}

/// What a relation line of a chip's popover is about.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RelationGlyph {
    /// Either/or: the cluster's other members (`or`).
    Or,
    /// A combined level (`Σ`).
    Sum,
    /// A stack of copies (`×`).
    Times,
}

/// One relation line of a chip's popover.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Relation {
    pub glyph: RelationGlyph,
    pub text: String,
}

/// What a chip shows of one item: everything drawn on it but its badges,
/// relations and state — the fields [`ChipView`] carries for its own row,
/// and [`ChipView::lifted`] for the item a drag of it carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChipFace {
    /// The short name beside the sprite: the item, or `Any melee`.
    pub name: String,
    /// The full title: the item, or `Any Tier 3+ melee weapon`.
    pub title: String,
    pub item: Option<ItemId>,
    pub kind: KindName,
    pub family: ItemKind,
    /// Qualifiers after the name, before the effect cue ([`chip_tags`]).
    pub tags: Vec<Tag>,
    /// Qualifiers after the effect cue ([`chip_trailing_tags`]).
    pub trailing_tags: Vec<Tag>,
    pub effect: Option<EffectBadge>,
    /// Whether cursed items are ruled out (drawn as a check mark).
    pub uncursed: bool,
    /// What the item is asked for, as parts ([`chip_details`]).
    pub details: Vec<String>,
    /// The accessibility label: the title, then the details.
    pub description: String,
}

impl ChipFace {
    /// The face of a chip asking for `requirement`; `counting_levels` when
    /// its stack counts levels, whose total speaks for the upgrades.
    #[must_use]
    pub fn of(requirement: &Requirement, counting_levels: bool) -> Self {
        let title = requirement_title(requirement);
        let details = chip_details(requirement, counting_levels);
        Self {
            name: requirement_name(requirement).to_owned(),
            description: chip_description(&title, &details),
            title,
            item: requirement.item,
            kind: KindName::of_requirement(requirement),
            family: requirement.kind,
            tags: chip_tags(requirement),
            trailing_tags: chip_trailing_tags(requirement),
            effect: effect_badge(requirement),
            uncursed: requirement.require_uncursed,
            details,
        }
    }
}

/// One chip: a visible row, alone or as a cluster member.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent facts about one chip.
pub struct ChipView {
    pub key: u64,
    /// The short name beside the sprite: the item, or `Any melee`.
    pub name: String,
    /// The full title the popover and the sheet lead with: the item, or
    /// `Any Tier 3+ melee weapon`.
    pub title: String,
    pub item: Option<ItemId>,
    pub kind: KindName,
    pub family: ItemKind,
    /// Qualifiers after the name, before the effect cue: transmutations or
    /// `choose at +3`, the tier (wildcards only), the upgrade, `F≤N`.
    pub tags: Vec<Tag>,
    /// Qualifiers after the effect cue: `No resin`.
    pub trailing_tags: Vec<Tag>,
    pub effect: Option<EffectBadge>,
    /// Whether cursed items are ruled out (drawn as a check mark).
    pub uncursed: bool,
    /// The popover's detail line, as parts: what the chip asks of its item.
    pub details: Vec<String>,
    /// The popover's relation lines: the cluster, the combined level, the
    /// stack.
    pub relations: Vec<Relation>,
    /// The badges the chip shows at rest.
    pub badges: Badges,
    /// The badges the chip keeps while one of its items is lifted away —
    /// what its drag origin shows, since every drag moves exactly one: the
    /// chip's own stack one item fewer, with the combined level
    /// [`super::Edit::RemoveOne`] of that item leaves. `None` when the chip
    /// has no copies: the whole chip leaves.
    pub remaining_badges: Option<Badges>,
    /// The face of the item a drag of the chip carries, when it has copies:
    /// a bare copy of it — its item, or its kind for a wildcard stack, with
    /// that copy's floor limit — which [`super::Edit::Join`] and
    /// [`super::Edit::Detach`] move while the chip keeps its constraints.
    /// `None` when the chip has no copies: the chip itself moves. The
    /// moving chip draws [`ChipView::moving_face`].
    pub lifted: Option<ChipFace>,
    /// The keys of the hidden copies behind the chip's badge. Members whose
    /// stacks are alike share theirs.
    pub copies: Vec<u64>,
    /// What the chip's count, combined-level and copy-floor steppers offer.
    pub stack: StackView,
    /// The accessibility label: the title, then the details.
    pub description: String,
    /// The chip's own first problem, else the first problem between rows
    /// that blames it; the chip also speaks for its hidden copies.
    pub problem: Option<String>,
    pub in_cluster: bool,
    /// Whether "On its own" ([`super::Edit::Detach`]) applies.
    pub can_detach: bool,
    /// The visible rows this chip may join ([`super::Edit::Join`]) — what
    /// "Either/or with…" menus, pick mode, accessibility actions and drag
    /// hover feedback offer.
    pub join: Vec<u64>,
    /// The visible rows a join onto is refused, with the reason to show.
    pub refuse: Vec<(u64, Refusal)>,
}

impl ChipView {
    /// The chip's own face: its name, title, tags, effect cue and details.
    #[must_use]
    pub fn face(&self) -> ChipFace {
        ChipFace {
            name: self.name.clone(),
            title: self.title.clone(),
            item: self.item,
            kind: self.kind,
            family: self.family,
            tags: self.tags.clone(),
            trailing_tags: self.trailing_tags.clone(),
            effect: self.effect.clone(),
            uncursed: self.uncursed,
            details: self.details.clone(),
            description: self.description.clone(),
        }
    }

    /// The face a drag's moving chip draws, without badges: the item it
    /// carries ([`ChipView::lifted`]), else the chip's own.
    #[must_use]
    pub fn moving_face(&self) -> ChipFace {
        self.lifted.clone().unwrap_or_else(|| self.face())
    }
}

/// The query's Arcane Resin condition, as the resin chip and the
/// requirement sheet read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResinState {
    pub amount: ResinAmount,
    pub filter: ArcaneResinFilter,
}

/// How much resin the query asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResinAmount {
    /// Enough to bring the kept wands to +3.
    Auto,
    AtLeast(u16),
}

/// The Arcane Resin chip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResinChip {
    /// Always [`ARCANE_RESIN`].
    pub name: String,
    /// `Auto` or `≥N` and `Mage +2` — the resin the chip counts, styled
    /// [`TagStyle::Credit`], `Auto` and `Mage +2` explained in their
    /// tooltips — then the donor filter `F≤N`.
    pub tags: Vec<Tag>,
    /// Whether donor wands must be uncursed (drawn as a check mark).
    pub uncursed: bool,
    /// The chip's hover text: the donors' source, the one filter no tag
    /// shows. `None` for any source.
    pub tooltip: Option<String>,
    /// The filter in words, like a chip's details.
    pub details: Vec<String>,
    /// The accessibility label: the name, then the details.
    pub description: String,
}

/// The chip tags of `requirement`, in the order they follow its name:
/// transmutations (`Transmute ≤N`) or `choose at +3`, the tier (`T3`,
/// `T3+`, `T≤3` — wildcards only; a named item is the tier it is), the
/// upgrade (`+3`, `+3↑`, drawn apart), the floor limit (`F≤N`). The effect
/// cue and [`chip_trailing_tags`] come after.
#[must_use]
pub fn chip_tags(requirement: &Requirement) -> Vec<Tag> {
    let mut tags = Vec::new();
    let transmutations = transmutations(requirement);
    if transmutations > 0 {
        tags.push(Tag::plain(transmutations_tag(transmutations)));
    }
    if requirement.select_trinket {
        tags.push(Tag::plain(SELECT_TRINKET));
    }
    if requirement.item.is_none()
        && let Some(tier) = tier_tag(requirement.tier)
    {
        tags.push(Tag::plain(tier));
    }
    if let Some(upgrade) = upgrade_tag(requirement.upgrade) {
        tags.push(Tag {
            text: upgrade,
            style: TagStyle::Upgrade,
            tooltip: None,
        });
    }
    if let Some(depth) = requirement.max_depth {
        tags.push(Tag::plain(floor_tag(depth)));
    }
    tags
}

/// The chip tags after the effect cue: `No resin`.
#[must_use]
pub fn chip_trailing_tags(requirement: &Requirement) -> Vec<Tag> {
    if requirement.exclude_resin {
        vec![Tag::plain(NO_RESIN)]
    } else {
        Vec::new()
    }
}

/// The transmutation limit a trinket or artifact carries, whichever field
/// holds it (the web reads the trinket's first).
fn transmutations(requirement: &Requirement) -> u8 {
    if requirement.trinket_transmutations > 0 {
        requirement.trinket_transmutations
    } else {
        requirement.artifact_transmutations
    }
}

/// The effect cue of `requirement`, or `None` for the wildcard.
#[must_use]
pub fn effect_badge(requirement: &Requirement) -> Option<EffectBadge> {
    let EffectRequirement::OneOf(set) = requirement.effect else {
        return None;
    };
    Some(EffectBadge {
        label: effect_label(requirement.effect)?,
        effects: set.effects().collect(),
        any_enchantment: EffectSet::enchantments(set.family()) == Some(set),
        curses_only: set.is_curses_only(),
    })
}

/// What `requirement` asks of its item, as the popover's detail parts:
/// the transmutation limit, `choose at +3`, the upgrade, the effect,
/// `uncursed`, the resin exclusion, the source, the floor limit.
///
/// "any upgrade" is left out when it says nothing: on a stack counting
/// levels, whose total speaks for the upgrades, and on trinkets and
/// artifacts, which are never upgraded in a search.
#[must_use]
pub fn chip_details(requirement: &Requirement, counting_levels: bool) -> Vec<String> {
    let mut details = Vec::new();
    let transmutations = transmutations(requirement);
    if transmutations > 0 {
        details.push(transmutations_detail(transmutations));
    }
    if requirement.select_trinket {
        details.push(SELECT_TRINKET.to_owned());
    }
    let silent = requirement.upgrade == UpgradeRequirement::Any
        && (counting_levels || matches!(requirement.kind, ItemKind::Trinket | ItemKind::Artifact));
    if !silent {
        details.push(upgrade_detail(requirement.upgrade));
    }
    if let Some(effect) = effect_label(requirement.effect) {
        details.push(effect);
    }
    if requirement.require_uncursed {
        details.push(UNCURSED.to_owned());
    }
    if requirement.exclude_resin {
        details.push(EXCLUDED_FROM_RESIN.to_owned());
    }
    if let Some(source) = requirement.source {
        details.push(source.label().to_owned());
    }
    if let Some(depth) = requirement.max_depth {
        details.push(floor_detail(depth));
    }
    details
}

/// An accessibility label: `title`, then `details` — `Rat Skull, within 3
/// transmutations`.
#[must_use]
pub fn chip_description(title: &str, details: &[String]) -> String {
    if details.is_empty() {
        title.to_owned()
    } else {
        format!("{title}, {}", details.join(", "))
    }
}

/// The popover's relation lines for the chip of `stack`, a member of
/// `item`: the cluster's other members, then its combined level or its
/// stack.
fn relations(rows: &[Row], item: &BoardItem, stack: &ChipStack) -> Vec<Relation> {
    let mut relations = Vec::new();
    if item.cluster.is_some() {
        let peers: Vec<&str> = item
            .members
            .iter()
            .filter(|&&member| member != stack.index)
            .map(|&member| requirement_name(&rows[member].requirement))
            .collect();
        relations.push(Relation {
            glyph: RelationGlyph::Or,
            text: peers.join(", "),
        });
    }
    let count = u8::try_from(stack.count()).unwrap_or(u8::MAX);
    let depths: BTreeSet<Option<u8>> = stack
        .copies
        .iter()
        .map(|&copy| rows[copy].requirement.max_depth)
        .collect();
    let floors = match depths.iter().copied().collect::<Vec<_>>()[..] {
        [Some(depth)] => CopyFloors::Within(depth),
        [None] => CopyFloors::Any,
        _ => CopyFloors::Own,
    };
    if let Some(total) = stack.total {
        // A counting stack's copies keep their own floor limits; the line
        // names them where the chip's floor tag would not say them.
        let anchor_depth = rows[stack.index].requirement.max_depth;
        let copies = (depths.iter().any(|&depth| depth != anchor_depth)).then_some(floors);
        relations.push(Relation {
            glyph: RelationGlyph::Sum,
            text: level_sum_relation(count, total, copies),
        });
    } else if count > 1 {
        relations.push(Relation {
            glyph: RelationGlyph::Times,
            text: stack_relation(count, floors),
        });
    }
    relations
}

/// The badges the chip of `stack` shows at rest.
fn badges(stack: &ChipStack) -> Badges {
    let count = u8::try_from(stack.count()).unwrap_or(u8::MAX);
    let counting = stack.total.is_some();
    Badges {
        count: (count > 1).then(|| {
            let text = count_text(count, counting);
            Badge {
                compact_text: text.clone(),
                text,
                tooltip: count_tooltip(count, counting),
            }
        }),
        total: stack.total.map(|total| Badge {
            text: total_text(total),
            compact_text: compact_total_text(total),
            tooltip: total_tooltip(total),
        }),
    }
}

/// The badges the chip of `stack` keeps while one of its items is lifted
/// away ([`ChipView::remaining_badges`]): its own stack one item fewer,
/// with the combined level [`Edit::RemoveOne`] of that item, beside the
/// `held` labels, leaves it — capped at what the rest can reach, dropped at
/// one ring — so the two agree. The count is the chip's own even where the
/// removal would fold what is left into another chip, which only a list
/// never normalized allows (a `Mace` stacked with a bare copy, beside a
/// lone `Mace`): a join onto that lone Mace leaves the rest apart. A
/// refused removal leaves the badges as they are. `None` when the chip has
/// no copies: the whole chip leaves.
fn remaining_badges(rows: &[Row], stack: &ChipStack, held: &HeldLabels) -> Option<Badges> {
    if stack.copies.is_empty() {
        return None;
    }
    let key = rows[stack.index].key;
    let result = apply_holding(rows, None, &[Edit::RemoveOne { key }], held);
    if result.refused.is_some() {
        return Some(badges(stack));
    }
    let mut rest = stack.clone();
    rest.copies.pop();
    rest.total = if rest.copies.is_empty() {
        None
    } else {
        let left = result.rows.iter().position(|row| row.key == key);
        left.and_then(|index| {
            board_items(&result.rows)
                .iter()
                .find_map(|item| item.stack(index))
                .map(|left| left.total)
        })
        .unwrap_or(stack.total)
    };
    Some(badges(&rest))
}

/// The resin chip for the query's resin condition.
#[must_use]
pub fn resin_chip(resin: &ResinState) -> ResinChip {
    let filter = resin.filter;
    let (amount_tag, amount_detail) = match resin.amount {
        ResinAmount::Auto => (RESIN_AUTO.to_owned(), RESIN_AUTO.to_owned()),
        ResinAmount::AtLeast(amount) => (format!("≥{amount}"), format!("at least {amount}")),
    };
    let mut tags = vec![Tag {
        text: amount_tag,
        style: TagStyle::Credit,
        tooltip: (resin.amount == ResinAmount::Auto).then(|| RESIN_AUTO_TOOLTIP.to_owned()),
    }];
    if filter.include_mage_wand {
        tags.push(Tag {
            text: RESIN_MAGE_TAG.to_owned(),
            style: TagStyle::Credit,
            tooltip: Some(RESIN_MAGE_TOOLTIP.to_owned()),
        });
    }
    if let Some(depth) = filter.max_depth {
        tags.push(Tag::plain(floor_tag(depth)));
    }
    let mut details = vec![amount_detail];
    if filter.include_mage_wand {
        details.push(RESIN_MAGE_DETAIL.to_owned());
    }
    details.push(
        if filter.uncursed {
            "uncursed wands"
        } else {
            "any wands"
        }
        .to_owned(),
    );
    if let Some(source) = filter.source {
        details.push(source.label().to_owned());
    }
    if let Some(depth) = filter.max_depth {
        details.push(floor_detail(depth));
    }
    ResinChip {
        name: ARCANE_RESIN.to_owned(),
        tags,
        uncursed: filter.uncursed,
        tooltip: filter.source.map(|source| source.label().to_owned()),
        description: chip_description(ARCANE_RESIN, &details),
        details,
    }
}

/// Which problem each row's chip can show: for every row, the first
/// problem of its own and the first problem between rows that blames it.
struct Blame<'a> {
    /// The first own (row-scope) problem of each row.
    own: Vec<Option<&'a str>>,
    /// The first problem between rows that blames each row.
    shared: Vec<Option<&'a str>>,
}

impl<'a> Blame<'a> {
    fn new(len: usize, found: &'a [IndexedProblem]) -> Self {
        let mut blame = Self {
            own: vec![None; len],
            shared: vec![None; len],
        };
        for problem in found {
            let slots = match problem.scope {
                ProblemScope::Row => &mut blame.own,
                ProblemScope::Group => &mut blame.shared,
                ProblemScope::List => continue,
            };
            for &index in &problem.rows {
                slots[index].get_or_insert(problem.message.as_str());
            }
        }
        blame
    }

    /// The chip problem of the row at `index`: its own, else a shared one;
    /// `copies` (the chip's hidden copies) speak after it, own problems
    /// first.
    fn chip(&self, index: usize, copies: &[usize]) -> Option<&'a str> {
        self.own[index]
            .or(self.shared[index])
            .or_else(|| copies.iter().find_map(|&copy| self.own[copy]))
            .or_else(|| copies.iter().find_map(|&copy| self.shared[copy]))
    }
}

/// The whole board: every entry with its chips, badges and stack controls,
/// the section counts, the list's problems, and the resin chip when `resin`
/// is set. `rows` are shown as they are; platforms normalize a list they
/// import ([`super::Edit::Normalize`]).
#[must_use]
pub fn board_view(rows: &[Row], resin: Option<&ResinState>) -> BoardView {
    board_view_beside(rows, resin, (Unread::default(), &HeldLabels::default()))
}

/// [`board_view`] of a list that also holds rows the editor cannot read,
/// in the `unread` sections, holding the `held` labels: they count towards
/// the list-level problem, and no join the chips offer takes their labels.
pub(crate) fn board_view_beside(
    rows: &[Row],
    resin: Option<&ResinState>,
    (unread, held): (Unread, &HeldLabels),
) -> BoardView {
    let items = board_items(rows);
    let candidates = join_candidates_holding(rows, &items, held);
    let found = indexed_problems(rows, unread);
    let blame = Blame::new(rows.len(), &found);
    let mut counts = Counts::default();
    let views = items
        .iter()
        .map(|item| {
            let anchor = &rows[item.anchor()].requirement;
            if anchor.blanket {
                counts.blanket += 1;
            } else {
                counts.ordinary += 1;
            }
            item_view(rows, item, &candidates, &found, &blame, held)
        })
        .collect();
    BoardView {
        items: views,
        counts,
        problems: keyed(rows, found),
        resin: resin.map(resin_chip),
    }
}

/// The view of `item`, one of [`board_items`] of `rows`.
fn item_view(
    rows: &[Row],
    item: &BoardItem,
    candidates: &[JoinCandidates],
    found: &[IndexedProblem],
    blame: &Blame<'_>,
    held: &HeldLabels,
) -> ItemView {
    let keys = |indices: &[usize]| indices.iter().map(|&index| rows[index].key).collect();
    let chips: Vec<ChipView> = item
        .stacks
        .iter()
        .map(|stack| {
            chip_view(
                rows,
                item,
                stack,
                &candidates[stack.index],
                blame.chip(stack.index, &stack.copies).map(str::to_owned),
                held,
            )
        })
        .collect();
    let problem = found
        .iter()
        .find(|problem| {
            problem
                .rows
                .iter()
                .any(|index| item.members.contains(index) || item.extras.contains(index))
        })
        .map(|problem| problem.message.clone());
    ItemView {
        id: item.key(rows),
        blanket: rows[item.anchor()].requirement.blanket,
        cluster: item.cluster,
        label: item.cluster.map(|_| alternatives_label(item.members.len())),
        name: entry_name(chips.iter().map(|chip| chip.name.as_str())),
        members: keys(&item.members),
        extras: keys(&item.extras),
        chips,
        problem,
    }
}

/// The chip of `stack`, a member of `item`.
fn chip_view(
    rows: &[Row],
    item: &BoardItem,
    stack: &ChipStack,
    candidates: &JoinCandidates,
    problem: Option<String>,
    held: &HeldLabels,
) -> ChipView {
    let row = &rows[stack.index];
    let ChipFace {
        name,
        title,
        item: item_id,
        kind,
        family,
        tags,
        trailing_tags,
        effect,
        uncursed,
        details,
        description,
    } = ChipFace::of(&row.requirement, stack.total.is_some());
    ChipView {
        key: row.key,
        name,
        title,
        item: item_id,
        kind,
        family,
        tags,
        trailing_tags,
        effect,
        uncursed,
        details,
        description,
        relations: relations(rows, item, stack),
        badges: badges(stack),
        remaining_badges: remaining_badges(rows, stack, held),
        lifted: lifted(rows, row.key).map(|carried| ChipFace::of(&carried, false)),
        copies: stack.copies.iter().map(|&copy| rows[copy].key).collect(),
        stack: stack_view(rows, stack),
        problem,
        in_cluster: item.cluster.is_some(),
        can_detach: item.cluster.is_some(),
        join: candidates.join.clone(),
        refuse: candidates.refuse.clone(),
    }
}

#[cfg(test)]
mod tests;
