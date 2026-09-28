//! Stack rules: what a chip's count and combined-level badges offer. Every
//! chip — a lone chip or a cluster member — has a stack of its own.
//!
//! Every platform draws the badges and steppers itself; these functions say
//! what they may offer, so a stepper never proposes an edit [`super::apply`]
//! would refuse or ignore.

use crate::catalog::ItemKind;
use crate::query::{Requirement, SumGroup, UpgradeRequirement};

use super::labels::{count_text, total_text};
use super::{ChipStack, STACK_MAX};

/// Whether the chip can grow a stack (web `canStack`): every chip — a lone
/// chip or a cluster member — has a stack of its own, whose copies name its
/// own kind. Trinkets and artifacts are unique finds and never stack, and a
/// blanket requirement constrains items the ordinary ones reserve rather
/// than reserving more (the engine rejects a stack label on one).
#[must_use]
pub fn can_grow<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> bool {
    let chip = rows[stack.index].as_ref();
    !chip.blanket && !matches!(chip.kind, ItemKind::Trinket | ItemKind::Artifact)
}

/// Whether the count stepper is live: the chip can grow, or it holds
/// copies to shed.
#[must_use]
pub fn can_change_count<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> bool {
    can_grow(rows, stack) || stack.count() > 1
}

/// The highest count the chip's stepper offers: [`STACK_MAX`] while it can
/// grow, else its own count, which it may only shed copies from. Never
/// above [`STACK_MAX`]: a hand-written stack of more items shrinks to it.
#[must_use]
pub fn count_max<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> u8 {
    if can_grow(rows, stack) {
        STACK_MAX
    } else {
        u8::try_from(stack.count())
            .unwrap_or(u8::MAX)
            .min(STACK_MAX)
    }
}

/// Whether the chip may count its items' levels together: a lone chip of a
/// named ring with copies, since levels add up across rings alone and a
/// combined level cannot sit in a cluster. A stack already counting levels
/// reports `true` so it can be turned off.
#[must_use]
pub fn can_count_levels<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> bool {
    let anchor = rows[stack.index].as_ref();
    !stack.in_cluster
        && !anchor.blanket
        && anchor.item.is_some()
        && (stack.total.is_some() || (stack.count() > 1 && anchor.kind == ItemKind::Ring))
}

/// The largest total the chip's combined level can ask for.
///
/// With a total set it is the level-sum group's
/// [`SumGroup::attainable_capacity`]; without one, the capacity the stack
/// would have once counting — its items as any-upgrade copies of the anchor
/// — so the stepper's range does not move when the switch is turned on. For
/// rings that is `(MAX_GENERATED_UPGRADE + 1) + (count − 1) ×
/// (MAX_STANDARD_RING_UPGRADE + 1)`: a world levels only one ring past the
/// standard roll.
#[must_use]
pub fn level_capacity<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> u8 {
    let anchor = *rows[stack.index].as_ref();
    let group = if let (Some(_), Some(sum)) = (stack.total, anchor.level_sum) {
        let mut group = SumGroup::default();
        for row in rows {
            let row = row.as_ref();
            if row
                .level_sum
                .is_some_and(|member| member.group == sum.group)
            {
                group.members = group.members.saturating_add(1);
                group.capacity = group
                    .capacity
                    .saturating_add(u16::from(row.maximum_level()));
            }
        }
        group
    } else {
        let members = u16::try_from(stack.count()).unwrap_or(u16::MAX);
        let copy = Requirement {
            upgrade: UpgradeRequirement::Any,
            ..anchor
        };
        SumGroup {
            members,
            minimum_total: 0,
            capacity: members.saturating_mul(u16::from(copy.maximum_level())),
        }
    };
    u8::try_from(group.attainable_capacity()).unwrap_or(u8::MAX)
}

/// The total a stack starts counting at: one level per item, within the
/// capacity.
#[must_use]
pub fn default_total<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> u8 {
    u8::try_from(stack.count())
        .unwrap_or(u8::MAX)
        .clamp(1, level_capacity(rows, stack).max(1))
}

/// The floor limit the chip's hidden copies share: the first copy's, when
/// a hand-written document gave them different ones.
#[must_use]
pub fn copy_depth<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> Option<u8> {
    stack
        .copies
        .first()
        .and_then(|&index| rows[index].as_ref().max_depth)
}

/// Whether the copies' floor limit is editable: there are copies, and the
/// stack is not counting levels — its members keep the limits they had
/// when it started, through to when it stops.
#[must_use]
pub fn can_set_copy_depth(stack: &ChipStack) -> bool {
    stack.count() > 1 && stack.total.is_none()
}

/// Everything a chip's count and combined-level badges and steppers show.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // Independent capabilities of one badge pair.
pub struct StackView {
    /// How many items the chip asks for, its own row included.
    pub count: u8,
    /// The most items one chip may ask for, [`STACK_MAX`].
    pub max: u8,
    pub can_grow: bool,
    pub can_change_count: bool,
    /// The count stepper's upper bound: `max` while the chip can grow, else
    /// its own count — a trinket or blanket may only shed copies
    /// ([`count_max`]).
    pub count_max: u8,
    /// The combined level, when the stack counts levels.
    pub total: Option<u8>,
    pub can_count_levels: bool,
    /// The total stepper's upper bound ([`level_capacity`]).
    pub level_capacity: u8,
    /// Where the total stepper starts when counting is turned on.
    pub default_total: u8,
    /// The hidden copies' floor limit.
    pub copy_depth: Option<u8>,
    pub can_set_copy_depth: bool,
    /// The count badge's text, `×N` (or `≤N` while counting levels, the
    /// members being optional) — shown by steppers even at ×1.
    pub count_text: String,
    /// The total badge's text, `Σ ≥ T` (`Σ ≥ 0` while a stepper edits a
    /// chip without a total yet).
    pub total_text: String,
}

/// The [`StackView`] of `stack`, the stack of a chip of
/// [`super::board_items`] of `rows`.
#[must_use]
pub fn stack_view<R: AsRef<Requirement>>(rows: &[R], stack: &ChipStack) -> StackView {
    let count = u8::try_from(stack.count()).unwrap_or(u8::MAX);
    StackView {
        count,
        max: STACK_MAX,
        can_grow: can_grow(rows, stack),
        can_change_count: can_change_count(rows, stack),
        count_max: count_max(rows, stack),
        total: stack.total,
        can_count_levels: can_count_levels(rows, stack),
        level_capacity: level_capacity(rows, stack),
        default_total: default_total(rows, stack),
        copy_depth: copy_depth(rows, stack),
        can_set_copy_depth: can_set_copy_depth(stack),
        count_text: count_text(count, stack.total.is_some()),
        total_text: total_text(stack.total.unwrap_or(0)),
    }
}
