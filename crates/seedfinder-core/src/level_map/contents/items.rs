//! Sprite identities from the pinned `ItemSpriteSheet`; appearance decks are run-specific.
use super::{MapItem, name};
use crate::catalog::{ItemId, item};
use crate::generator::{GeneratedItem, SeedKind, StoneKind};
use crate::level::{DirectPaintItem, PaintItem};
use crate::regular_items::{QueuedItemKind, RegularItem};
use crate::run::{ItemAppearanceState, PotionKind, ScrollKind};
use crate::special_equipment::SpecialItem;

pub(super) fn equipment(id: ItemId, quantity: i32, a: &ItemAppearanceState) -> MapItem {
    let definition = item(id);
    MapItem::new(
        definition.stable_id,
        definition.sprite_index_in(a.ring_gems),
        quantity,
    )
}
pub(super) fn generated(value: GeneratedItem, a: &ItemAppearanceState) -> MapItem {
    if let Some(e) = value.searchable_equipment() {
        let quantity = match value {
            GeneratedItem::Missile(m) => m.quantity,
            GeneratedItem::TippedDart { quantity, .. } => quantity,
            _ => 1,
        };
        return with_glow(
            equipment(e.item, quantity, a),
            e.item,
            e.roll.effect,
            e.roll.cursed,
        );
    }
    match value {
        GeneratedItem::Food(kind) => MapItem::new(name(kind), [437, 438, 432][kind as usize], 1),
        GeneratedItem::Potion { kind, exotic } => potion(kind, exotic, a),
        GeneratedItem::Scroll { kind, exotic } => scroll(kind, exotic, a),
        GeneratedItem::Seed(kind) => {
            MapItem::new(format!("{kind:?}Seed"), 384 + seed_image(kind), 1)
        }
        GeneratedItem::Stone(kind) => stone(kind),
        GeneratedItem::Gold { quantity } => MapItem::new("Gold", 18, quantity),
        GeneratedItem::Trinket(kind) => MapItem::new(name(kind), 272 + kind as u16, 1),
        GeneratedItem::Bomb(kind) => MapItem::new(
            name(kind),
            match kind {
                crate::generator::BombKind::Bomb => 80,
                crate::generator::BombKind::DoubleBomb => 81,
            },
            1,
        ),
        GeneratedItem::Missile(m) => MapItem::new("Dart", 160, m.quantity),
        GeneratedItem::Artifact(artifact) => MapItem::new(
            name(artifact.kind),
            match artifact.kind {
                crate::generator::ArtifactKind::CloakOfShadows => 240,
                _ => 263,
            },
            1,
        ),
        GeneratedItem::Equipment(_) | GeneratedItem::Ring(_) | GeneratedItem::TippedDart { .. } => {
            unreachable!("searchable equipment was handled above")
        }
    }
}
fn potion(kind: PotionKind, exotic: bool, a: &ItemAppearanceState) -> MapItem {
    MapItem::new(
        format!("{}PotionOf{kind:?}", if exotic { "Exotic" } else { "" }),
        352 + u16::from(exotic) * 16 + a.potion_color(kind) as u16,
        1,
    )
}
fn scroll(kind: ScrollKind, exotic: bool, a: &ItemAppearanceState) -> MapItem {
    MapItem::new(
        format!("{}ScrollOf{kind:?}", if exotic { "Exotic" } else { "" }),
        304 + u16::from(exotic) * 16 + a.scroll_label(kind) as u16,
        1,
    )
}
pub(super) const fn seed_image(kind: SeedKind) -> u16 {
    match kind {
        SeedKind::Rotberry => 0,
        SeedKind::Firebloom => 1,
        SeedKind::Swiftthistle => 2,
        SeedKind::Sungrass => 3,
        SeedKind::Icecap => 4,
        SeedKind::Stormvine => 5,
        SeedKind::Sorrowmoss => 6,
        SeedKind::Mageroyal => 7,
        SeedKind::Earthroot => 8,
        SeedKind::Starflower => 9,
        SeedKind::Fadeleaf => 10,
        SeedKind::Blindweed => 11,
    }
}
fn stone(kind: StoneKind) -> MapItem {
    let offset = match kind {
        StoneKind::Aggression => 0,
        StoneKind::Augmentation => 1,
        StoneKind::Fear => 2,
        StoneKind::Blast => 3,
        StoneKind::Blink => 4,
        StoneKind::Clairvoyance => 5,
        StoneKind::DeepSleep => 6,
        StoneKind::DetectMagic => 7,
        StoneKind::Enchantment => 8,
        StoneKind::Flock => 9,
        StoneKind::Intuition => 10,
        StoneKind::Shock => 11,
    };
    MapItem::new(format!("StoneOf{kind:?}"), 336 + offset, 1)
}

// Constructor-only classes already carry their identity in the generation log.
// An unrecognized class is explicit, never silently omitted from the map.
pub(super) fn direct(kind: &str, a: &ItemAppearanceState) -> MapItem {
    let potions = [
        PotionKind::Strength,
        PotionKind::Healing,
        PotionKind::MindVision,
        PotionKind::Frost,
        PotionKind::LiquidFlame,
        PotionKind::ToxicGas,
        PotionKind::Haste,
        PotionKind::Invisibility,
        PotionKind::Levitation,
        PotionKind::ParalyticGas,
        PotionKind::Purity,
        PotionKind::Experience,
    ];
    if let Some(p) = potions
        .into_iter()
        .find(|p| format!("PotionOf{p:?}") == kind)
    {
        return potion(p, false, a);
    }
    let scrolls = [
        ScrollKind::Upgrade,
        ScrollKind::Identify,
        ScrollKind::RemoveCurse,
        ScrollKind::MirrorImage,
        ScrollKind::Recharging,
        ScrollKind::Teleportation,
        ScrollKind::Lullaby,
        ScrollKind::MagicMapping,
        ScrollKind::Rage,
        ScrollKind::Retribution,
        ScrollKind::Terror,
        ScrollKind::Transmutation,
    ];
    if let Some(s) = scrolls
        .into_iter()
        .find(|s| format!("ScrollOf{s:?}") == kind)
    {
        return scroll(s, false, a);
    }
    let image = match kind {
        "Gold" => 18,
        "EnergyCrystal" => 19,
        "HourglassSandBag" => 23,
        "Ankh" => 48,
        "ArcaneStylus" | "Stylus" => 49,
        "Torch" => 51,
        "Honeypot" => 53,
        "ShatteredPot" => 54,
        "IronKey" => 55,
        "GoldenKey" => 56,
        "CrystalKey" => 57,
        "TrinketCatalyst" => 70,
        "Dart" => 160,
        "StoneOfAugmentation" => 337,
        "StoneOfEnchantment" => 344,
        "StoneOfIntuition" => 346,
        "Alchemize" => 422,
        "ChargrilledMeat" => 433,
        "SmallRation" => 435,
        "CorpseDust" => 465,
        "CeremonialCandle" => 466,
        "ElementalEmbers" => 467,
        "DarkGold" => 469,
        "DwarfToken" => 470,
        "VaultBeacon" => 425,
        "ImpStatue" => 474,
        "VelvetPouch" => 482,
        "ScrollHolder" => 483,
        "PotionBandolier" => 484,
        "MagicalHolster" => 485,
        "GuidePage" | "Guidebook$GuidePage" => 496,
        "AlchemyPage" => 497,
        "RegionLorePage$Sewers" => 498,
        "RegionLorePage$Prison" => 499,
        "RegionLorePage$Caves" => 500,
        "RegionLorePage$City" => 501,
        "RegionLorePage$Halls" => 502,
        _ => 0,
    };
    let mut item = MapItem::new(kind, image, 1);
    // VaultBeacon.glowing(): unconditional white, using ItemSprite.Glowing's
    // default one-second fade in (and one-second fade out).
    if kind == "VaultBeacon" {
        item.glow = Some(crate::level_map::MapGlow {
            color: [255, 255, 255],
            period_ms: 1000,
        });
    }
    item
}
pub(super) fn paint(value: PaintItem, a: &ItemAppearanceState) -> MapItem {
    match value {
        PaintItem::Generated(i) => generated(i, a),
        PaintItem::Direct(DirectPaintItem::Other(s)) => direct(s, a),
        PaintItem::Direct(i) => direct(&name(i), a),
        i => direct(&name(i), a),
    }
}
pub(super) fn regular(value: RegularItem, a: &ItemAppearanceState) -> MapItem {
    match value {
        RegularItem::Generated(i) => generated(i, a),
        RegularItem::Queued(QueuedItemKind::Other(s)) => direct(s, a),
        RegularItem::Queued(i) => direct(&name(i), a),
    }
}
pub(super) fn special(value: SpecialItem, a: &ItemAppearanceState) -> MapItem {
    match value {
        SpecialItem::Paint(i) => paint(i, a),
        i => direct(&name(i), a),
    }
}
pub(super) fn consumable(
    value: crate::special_consumable::ConsumableItem,
    a: &ItemAppearanceState,
) -> MapItem {
    match value {
        crate::special_consumable::ConsumableItem::Special(i) => special(i, a),
        i @ crate::special_consumable::ConsumableItem::Honeypot => direct(&name(i), a),
    }
}
pub(super) fn secret(value: crate::secret_rooms::SecretItem, a: &ItemAppearanceState) -> MapItem {
    use crate::secret_rooms::SecretItem as I;
    match value {
        I::Generated(i) => generated(i, a),
        I::EnergyCrystal { quantity } => MapItem::new("EnergyCrystal", 19, quantity),
        i => direct(&name(i), a),
    }
}
pub(super) fn forced(
    value: &crate::special_forced::ForcedItem,
    a: &ItemAppearanceState,
    depth: u32,
) -> MapItem {
    use crate::shop::{DirectShopItem as D, ShopStockItem as S};
    use crate::special_forced::ForcedItem as I;
    match value {
        I::Regular(i) => regular(*i, a),
        I::EnergyCrystal { quantity } => MapItem::new("EnergyCrystal", 19, *quantity),
        I::AlchemyPage(_) => direct("AlchemyPage", a),
        I::Shop(S::Searchable(i)) => with_glow(
            equipment(
                i.item,
                if i.item.is_tipped_dart() {
                    2
                } else if crate::catalog::item(i.item).weapon_category()
                    == Some(crate::catalog::WeaponCategory::Thrown)
                {
                    3
                } else {
                    1
                },
                a,
            ),
            i.item,
            i.effect,
            i.cursed,
        ),
        I::Shop(S::Generated(i)) => generated(*i, a),
        I::Shop(S::Direct(D::Bag(_))) => {
            // Preview the requested purchase sequence without changing shop RNG,
            // stock order, or the search engine's inventory assumptions.
            let kind = match depth {
                0..=10 => "ScrollHolder",
                11..=15 => "PotionBandolier",
                _ => "MagicalHolster",
            };
            MapItem {
                deterministic: false,
                ..direct(kind, a)
            }
        }
        I::Shop(S::Direct(D::Alchemize { quantity })) => MapItem::new("Alchemize", 422, *quantity),
        I::Shop(S::Direct(i)) => direct(&name(i), a),
    }
}
pub(super) fn vault(value: crate::vault_loot::VaultItem, a: &ItemAppearanceState) -> MapItem {
    use crate::vault_loot::{VaultConsumable as C, VaultItem as I};
    match value {
        I::Equipment(i) => with_glow(equipment(i.item, i.quantity, a), i.item, i.effect, false),
        I::Dart => MapItem::new("Dart", 160, 2),
        I::Consumable(c) => generated(
            match c {
                C::Potion(kind) => GeneratedItem::Potion {
                    kind,
                    exotic: false,
                },
                C::Scroll(kind) => GeneratedItem::Scroll {
                    kind,
                    exotic: false,
                },
                C::Seed(kind) => GeneratedItem::Seed(kind),
                C::Stone(kind) => GeneratedItem::Stone(kind),
            },
            a,
        ),
        I::ShuffledConsumable { .. } => MapItem::unknown("RuntimeVaultConsumable", 0),
        I::Food(kind) => generated(GeneratedItem::Food(kind), a),
        I::ImpRewardOption(_) => MapItem::unknown("ImpRewardOption", 0),
        i => direct(&name(i), a),
    }
}

pub(super) fn imp(value: crate::quests::ImpRewardOption, a: &ItemAppearanceState) -> MapItem {
    use crate::quests::ImpRewardOption as I;
    let mut item = generated(
        match value {
            I::Artifact(v) => GeneratedItem::Artifact(v),
            I::Ring(v) => GeneratedItem::Ring(v),
            I::Equipment(v) => GeneratedItem::Equipment(v),
            I::Missile(v) => GeneratedItem::Missile(v),
        },
        a,
    );
    // The vault transfers +5 into its artifact option before dropping it.
    item.image += match item.kind.as_str() {
        "sandals_of_nature" => 2,
        "chalice_of_blood" | "dried_rose" => 1,
        _ => 0,
    };
    item
}

fn with_glow(
    mut item: MapItem,
    id: ItemId,
    effect: Option<crate::catalog::Effect>,
    cursed: bool,
) -> MapItem {
    item.glow = crate::level_map::MapGlow::for_item(crate::catalog::item(id).kind, effect, cursed);
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ITEMS, ItemKind};
    use crate::equipment::EquipmentRoll;
    use crate::generator::GeneratedEquipment;
    use crate::run::RunState;

    #[test]
    fn generated_wands_never_have_a_map_glow() {
        let appearances = RunState::new(0).appearances;
        for wand in ITEMS.iter().filter(|i| i.kind == ItemKind::Wand) {
            for cursed in [false, true] {
                let value = GeneratedItem::Equipment(GeneratedEquipment {
                    item: wand.id,
                    roll: EquipmentRoll {
                        upgrade: 0,
                        effect: None,
                        cursed,
                    },
                });
                let sprite = generated(value, &appearances);
                assert_eq!(sprite.kind, wand.stable_id);
                assert_eq!(sprite.glow, None, "{} cursed={cursed}", wand.stable_id);
            }
        }
    }
}
