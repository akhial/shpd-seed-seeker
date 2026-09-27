//! The English words every frontend shows for a requirement: category and
//! kind names, wildcard names and titles, and the phrases chips, badges and
//! details are built from.
//!
//! The platforms each grew their own tables, which disagreed on casing,
//! tier wording and even which noun a wildcard used ("Any melee" on one
//! board, "Any melee weapon" on the next). One table here settles them; the
//! choices follow the shared design: title-case item names from the
//! catalog asset ([`crate::catalog::ItemDefinition::display_name`]),
//! sentence-case sources ([`crate::model::ItemSource::label`]), "Any Tier 3
//! weapon" titles and the short "Any melee" chip names.

use crate::catalog::{Effect, ItemKind, WeaponCategory, item};
use crate::query::{
    EffectRequirement, EffectSet, Requirement, TierRequirement, UpgradeRequirement,
};

/// A family as a category picker names it.
#[must_use]
pub const fn category_label(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Weapon => "Weapon",
        ItemKind::Armor => "Armor",
        ItemKind::Wand => "Wand",
        ItemKind::Ring => "Ring",
        ItemKind::Trinket => "Trinket",
        ItemKind::Artifact => "Artifact",
    }
}

/// A family as a group heading names it.
#[must_use]
pub const fn category_plural(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Weapon => "Weapons",
        ItemKind::Armor => "Armor",
        ItemKind::Wand => "Wands",
        ItemKind::Ring => "Rings",
        ItemKind::Trinket => "Trinkets",
        ItemKind::Artifact => "Artifacts",
    }
}

/// A family with its melee/thrown narrowing, as the flat kind picker names
/// it ("Melee weapon"). The narrowing only means something on weapons and
/// is ignored elsewhere.
#[must_use]
pub const fn kind_label(kind: ItemKind, category: Option<WeaponCategory>) -> &'static str {
    match (kind, category) {
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "Melee weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "Thrown weapon",
        _ => category_label(kind),
    }
}

/// [`kind_label`] as a noun inside a sentence ("melee weapon").
#[must_use]
pub const fn kind_noun(kind: ItemKind, category: Option<WeaponCategory>) -> &'static str {
    match (kind, category) {
        (ItemKind::Weapon, None) => "weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "melee weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "thrown weapon",
        (ItemKind::Armor, _) => "armor",
        (ItemKind::Wand, _) => "wand",
        (ItemKind::Ring, _) => "ring",
        (ItemKind::Trinket, _) => "trinket",
        (ItemKind::Artifact, _) => "artifact",
    }
}

/// The weapon-type picker's choices: any, melee or thrown.
#[must_use]
pub const fn weapon_type_label(category: Option<WeaponCategory>) -> &'static str {
    match category {
        None => "Any",
        Some(WeaponCategory::Melee) => "Melee",
        Some(WeaponCategory::Thrown) => "Thrown",
    }
}

/// The short name a chip shows for a requirement naming no item: the chip
/// already draws the family's icon, so "Any melee" says enough. Trinkets and
/// artifacts always name one; without it they read as their category.
#[must_use]
pub const fn wildcard_name(kind: ItemKind, category: Option<WeaponCategory>) -> &'static str {
    match (kind, category) {
        (ItemKind::Weapon, None) => "Any weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "Any melee",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "Any thrown",
        (ItemKind::Armor, _) => "Any armor",
        (ItemKind::Wand, _) => "Any wand",
        (ItemKind::Ring, _) => "Any ring",
        (ItemKind::Trinket, _) => "Trinket",
        (ItemKind::Artifact, _) => "Artifact",
    }
}

/// The item picker's wildcard option ("Any melee weapon"), which stands
/// among full item names and so spells the kind out.
#[must_use]
pub const fn wildcard_label(kind: ItemKind, category: Option<WeaponCategory>) -> &'static str {
    match (kind, category) {
        (ItemKind::Weapon, None) => "Any weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Melee)) => "Any melee weapon",
        (ItemKind::Weapon, Some(WeaponCategory::Thrown)) => "Any thrown weapon",
        (ItemKind::Armor, _) => "Any armor",
        (ItemKind::Wand, _) => "Any wand",
        (ItemKind::Ring, _) => "Any ring",
        (ItemKind::Trinket, _) => "Any trinket",
        (ItemKind::Artifact, _) => "Any artifact",
    }
}

/// The full title of a wildcard, with its tier spelled out: "Any Tier 3
/// weapon", "Any Tier 3+ melee weapon", "Any Tier 3 or lower armor".
#[must_use]
pub fn wildcard_title(
    kind: ItemKind,
    category: Option<WeaponCategory>,
    tier: TierRequirement,
) -> String {
    if matches!(kind, ItemKind::Trinket | ItemKind::Artifact) {
        return category_label(kind).to_owned();
    }
    let noun = kind_noun(kind, category);
    match tier {
        TierRequirement::Any => format!("Any {noun}"),
        TierRequirement::Exact(tier) => format!("Any Tier {tier} {noun}"),
        TierRequirement::AtLeast(tier) => format!("Any Tier {tier}+ {noun}"),
        TierRequirement::AtMost(tier) => format!("Any Tier {tier} or lower {noun}"),
    }
}

/// The short name a chip shows: the item's display name, or
/// [`wildcard_name`].
#[must_use]
pub fn requirement_name(requirement: &Requirement) -> &'static str {
    match requirement.item {
        Some(item_id) => item(item_id).display_name,
        None => wildcard_name(requirement.kind, requirement.weapon_category),
    }
}

/// The full title of a requirement: the item's display name, or
/// [`wildcard_title`].
#[must_use]
pub fn requirement_title(requirement: &Requirement) -> String {
    match requirement.item {
        Some(item_id) => item(item_id).display_name.to_owned(),
        None => wildcard_title(
            requirement.kind,
            requirement.weapon_category,
            requirement.tier,
        ),
    }
}

/// The tier as a chip tag: `T3`, `T3+` or `T≤3`. Only wildcards carry a
/// tier filter; a named item is the tier it is.
#[must_use]
pub fn tier_tag(tier: TierRequirement) -> Option<String> {
    match tier {
        TierRequirement::Any => None,
        TierRequirement::Exact(tier) => Some(format!("T{tier}")),
        TierRequirement::AtLeast(tier) => Some(format!("T{tier}+")),
        TierRequirement::AtMost(tier) => Some(format!("T≤{tier}")),
    }
}

/// The upgrade as a chip tag: `+3`, or `+3↑` for "at least".
#[must_use]
pub fn upgrade_tag(upgrade: UpgradeRequirement) -> Option<String> {
    match upgrade {
        UpgradeRequirement::Any => None,
        UpgradeRequirement::Exact(upgrade) => Some(format!("+{upgrade}")),
        UpgradeRequirement::AtLeast(upgrade) => Some(format!("+{upgrade}↑")),
    }
}

/// The upgrade as a detail phrase: `exactly +3`, `+3 or higher` or
/// `any upgrade`.
#[must_use]
pub fn upgrade_detail(upgrade: UpgradeRequirement) -> String {
    match upgrade {
        UpgradeRequirement::Any => "any upgrade".to_owned(),
        UpgradeRequirement::Exact(upgrade) => format!("exactly +{upgrade}"),
        UpgradeRequirement::AtLeast(upgrade) => format!("+{upgrade} or higher"),
    }
}

/// A floor limit as a chip tag: `F≤4`.
#[must_use]
pub fn floor_tag(max_depth: u8) -> String {
    format!("F≤{max_depth}")
}

/// A floor limit as a detail phrase: `floors 1–4`.
#[must_use]
pub fn floor_detail(max_depth: u8) -> String {
    format!("floors 1–{max_depth}")
}

/// A transmutation limit as a chip tag: `Transmute ≤3`.
#[must_use]
pub fn transmutations_tag(transmutations: u8) -> String {
    format!("Transmute ≤{transmutations}")
}

/// A transmutation limit as a detail phrase: `within 3 transmutations`.
#[must_use]
pub fn transmutations_detail(transmutations: u8) -> String {
    let plural = if transmutations == 1 { "" } else { "s" };
    format!("within {transmutations} transmutation{plural}")
}

/// The effect filter as text: `any enchantment` (or `any glyph` on armor)
/// for the full non-curse set, the effect's name for one, and
/// `effect: A/B/C` in catalog order for several. `None` for the wildcard.
#[must_use]
pub fn effect_label(effect: EffectRequirement) -> Option<String> {
    let EffectRequirement::OneOf(set) = effect else {
        return None;
    };
    if EffectSet::enchantments(set.family()) == Some(set) {
        return Some(
            if set.family() == ItemKind::Armor {
                "any glyph"
            } else {
                "any enchantment"
            }
            .to_owned(),
        );
    }
    let names: Vec<&str> = set.effects().map(Effect::wire_name).collect();
    Some(match names[..] {
        [name] => name.to_owned(),
        _ => format!("effect: {}", names.join("/")),
    })
}

/// The count badge: `×N`, or `≤N` for a stack counting levels, whose
/// members are optional ("up to N items").
#[must_use]
pub fn count_text(count: u8, counting_levels: bool) -> String {
    if counting_levels {
        format!("≤{count}")
    } else {
        format!("×{count}")
    }
}

/// The combined-level badge: `Σ ≥ 5`.
#[must_use]
pub fn total_text(total: u8) -> String {
    format!("Σ ≥ {total}")
}

/// A cluster's caption: `Any of 3`.
#[must_use]
pub fn alternatives_label(members: usize) -> String {
    format!("Any of {members}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ArmorEffect, ItemId, WeaponEffect};
    use crate::model::ItemSource;

    #[test]
    fn wildcards_are_named_short_on_chips_and_in_full_in_pickers_and_titles() {
        let melee = Some(WeaponCategory::Melee);
        let thrown = Some(WeaponCategory::Thrown);
        assert_eq!(wildcard_name(ItemKind::Weapon, None), "Any weapon");
        assert_eq!(wildcard_name(ItemKind::Weapon, melee), "Any melee");
        assert_eq!(wildcard_name(ItemKind::Weapon, thrown), "Any thrown");
        assert_eq!(wildcard_name(ItemKind::Ring, None), "Any ring");
        assert_eq!(wildcard_name(ItemKind::Trinket, None), "Trinket");
        assert_eq!(wildcard_label(ItemKind::Weapon, melee), "Any melee weapon");
        assert_eq!(wildcard_label(ItemKind::Armor, None), "Any armor");
        assert_eq!(kind_label(ItemKind::Weapon, thrown), "Thrown weapon");
        assert_eq!(kind_label(ItemKind::Wand, melee), "Wand");
        assert_eq!(weapon_type_label(thrown), "Thrown");
        assert_eq!(category_plural(ItemKind::Armor), "Armor");

        let tiers = [
            (TierRequirement::Any, "Any thrown weapon"),
            (TierRequirement::Exact(3), "Any Tier 3 thrown weapon"),
            (TierRequirement::AtLeast(3), "Any Tier 3+ thrown weapon"),
            (
                TierRequirement::AtMost(4),
                "Any Tier 4 or lower thrown weapon",
            ),
        ];
        for (tier, title) in tiers {
            assert_eq!(wildcard_title(ItemKind::Weapon, thrown, tier), title);
        }
        assert_eq!(
            wildcard_title(ItemKind::Artifact, None, TierRequirement::Any),
            "Artifact"
        );
    }

    #[test]
    fn a_named_requirement_reads_as_its_title_case_item() {
        let ring = Requirement {
            item: Some(ItemId::RingMight),
            ..Requirement::any(ItemKind::Ring)
        };
        assert_eq!(requirement_name(&ring), "Ring of Might");
        assert_eq!(requirement_title(&ring), "Ring of Might");
        let armor = Requirement {
            tier: TierRequirement::Exact(4),
            ..Requirement::any(ItemKind::Armor)
        };
        assert_eq!(requirement_name(&armor), "Any armor");
        assert_eq!(requirement_title(&armor), "Any Tier 4 armor");
    }

    #[test]
    fn tags_and_details_spell_each_bound() {
        assert_eq!(tier_tag(TierRequirement::Exact(3)).as_deref(), Some("T3"));
        assert_eq!(
            tier_tag(TierRequirement::AtLeast(3)).as_deref(),
            Some("T3+")
        );
        assert_eq!(tier_tag(TierRequirement::AtMost(4)).as_deref(), Some("T≤4"));
        assert_eq!(tier_tag(TierRequirement::Any), None);
        assert_eq!(
            upgrade_tag(UpgradeRequirement::Exact(2)).as_deref(),
            Some("+2")
        );
        assert_eq!(
            upgrade_tag(UpgradeRequirement::AtLeast(2)).as_deref(),
            Some("+2↑")
        );
        assert_eq!(upgrade_tag(UpgradeRequirement::Any), None);
        assert_eq!(upgrade_detail(UpgradeRequirement::Exact(3)), "exactly +3");
        assert_eq!(
            upgrade_detail(UpgradeRequirement::AtLeast(1)),
            "+1 or higher"
        );
        assert_eq!(upgrade_detail(UpgradeRequirement::Any), "any upgrade");
        assert_eq!(floor_tag(4), "F≤4");
        assert_eq!(floor_detail(9), "floors 1–9");
        assert_eq!(transmutations_tag(13), "Transmute ≤13");
        assert_eq!(transmutations_detail(1), "within 1 transmutation");
        assert_eq!(transmutations_detail(3), "within 3 transmutations");
        assert_eq!(count_text(3, false), "×3");
        assert_eq!(count_text(2, true), "≤2");
        assert_eq!(total_text(5), "Σ ≥ 5");
        assert_eq!(alternatives_label(3), "Any of 3");
    }

    #[test]
    fn effect_labels_name_one_effect_a_set_or_the_whole_family() {
        assert_eq!(effect_label(EffectRequirement::Any), None);
        let weapons = EffectSet::enchantments(ItemKind::Weapon).unwrap();
        assert_eq!(
            effect_label(EffectRequirement::OneOf(weapons)).as_deref(),
            Some("any enchantment")
        );
        let armor = EffectSet::enchantments(ItemKind::Armor).unwrap();
        assert_eq!(
            effect_label(EffectRequirement::OneOf(armor)).as_deref(),
            Some("any glyph")
        );
        let one = EffectRequirement::exactly(Effect::Armor(ArmorEffect::AntiMagic));
        assert_eq!(effect_label(one).as_deref(), Some("Anti-Magic"));
        // Several effects list in catalog order, whatever order they came in.
        let set = EffectSet::from_effects([
            Effect::Weapon(WeaponEffect::Grim),
            Effect::Weapon(WeaponEffect::Blazing),
            Effect::Weapon(WeaponEffect::Annoying),
        ])
        .unwrap();
        assert_eq!(
            effect_label(EffectRequirement::OneOf(set)).as_deref(),
            Some("effect: Blazing/Grim/Annoying")
        );
        // Every enchantment plus a curse is a set, not "any enchantment".
        let with_curse = EffectSet::from_effects(
            weapons
                .effects()
                .chain([Effect::Weapon(WeaponEffect::Wayward)]),
        )
        .unwrap();
        assert!(
            effect_label(EffectRequirement::OneOf(with_curse))
                .unwrap()
                .starts_with("effect: Blazing/")
        );
    }

    #[test]
    fn sources_read_in_sentence_case() {
        assert_eq!(ItemSource::LockedChest.label(), "Locked chest");
        assert_eq!(ItemSource::GhostReward.label(), "Ghost reward");
        assert_eq!(ItemSource::VaultTreasure.label(), "Vault treasure");
        for source in ItemSource::ALL {
            let label = source.label();
            // One capital, at the start: sentence case, never title case.
            assert!(
                label.starts_with(|c: char| c.is_ascii_uppercase()),
                "{label}"
            );
            assert!(
                !label[1..].contains(|c: char| c.is_ascii_uppercase()),
                "{label}"
            );
            // The label is the wire name made readable.
            assert_eq!(
                label.to_ascii_lowercase().replace(' ', "_"),
                crate::model::source_name(*source)
            );
        }
    }
}
