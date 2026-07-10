//! Canonical main-dungeon world generation through any depth 1..=24.
//!
//! The regional composites own the exact sequential state transitions for
//! their prefixes. This module is the stable search/JNI entry point and makes
//! the four boss-floor boundaries explicit.

use std::fmt;

use crate::caves_floor::{CanonicalCavesWorldGenerator, CavesFloorError, generate_caves_world};
use crate::city_floor::{CanonicalCityWorldGenerator, CityFloorError, generate_city_world};
use crate::halls_floor::{CanonicalHallsWorldGenerator, HallsFloorError, generate_halls_world};
use crate::model::GeneratedWorld;
use crate::prison_floor::{CanonicalPrisonWorldGenerator, PrisonFloorError, generate_prison_world};
use crate::search::WorldGenerator;
use crate::seed::DungeonSeed;
use crate::sewer_floor::{CanonicalSewerWorldGenerator, SewerFloorError, generate_sewer_world};

/// Exact version-pinned generator used by production search sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct CanonicalMainWorldGenerator;

impl WorldGenerator for CanonicalMainWorldGenerator {
    fn generate(&self, seed: DungeonSeed, max_depth: u8) -> GeneratedWorld {
        generate_main_world(seed, max_depth)
            .expect("CanonicalMainWorldGenerator accepts depths 1..=24")
    }

    fn generate_batch(&self, seeds: &[DungeonSeed], max_depth: u8) -> Vec<GeneratedWorld> {
        assert!(
            (1..=24).contains(&max_depth),
            "CanonicalMainWorldGenerator accepts depths 1..=24"
        );
        match max_depth {
            1..=4 => CanonicalSewerWorldGenerator.generate_batch(seeds, max_depth),
            5 => CanonicalSewerWorldGenerator.generate_batch(seeds, 4),
            6..=9 => CanonicalPrisonWorldGenerator.generate_batch(seeds, max_depth),
            10 => CanonicalPrisonWorldGenerator.generate_batch(seeds, 9),
            11..=14 => CanonicalCavesWorldGenerator.generate_batch(seeds, max_depth),
            15 => CanonicalCavesWorldGenerator.generate_batch(seeds, 14),
            16..=19 => CanonicalCityWorldGenerator.generate_batch(seeds, max_depth),
            20 => {
                let mut worlds = CanonicalHallsWorldGenerator.generate_batch(seeds, 21);
                for world in &mut worlds {
                    world.items.retain(|item| item.depth <= 20);
                }
                worlds
            }
            21..=24 => CanonicalHallsWorldGenerator.generate_batch(seeds, max_depth),
            _ => unreachable!("depth was validated above"),
        }
    }
}

/// Generates all searchable static equipment in the canonical main-dungeon
/// prefix through `maximum_depth`.
///
/// Boss floors 5, 10, and 15 contain no searchable initial equipment and are
/// persistent-state neutral in the pinned profile. Depth 20 is different: its
/// Imp shop eagerly generates stock, so the depth-21 Halls prefix is evaluated
/// and then trimmed when callers request exactly depth 20.
///
/// # Errors
///
/// Returns [`MainWorldError::InvalidMaximumDepth`] outside 1..=24, or wraps
/// the exact regional generation failure.
pub fn generate_main_world(
    seed: DungeonSeed,
    maximum_depth: u8,
) -> Result<GeneratedWorld, MainWorldError> {
    match maximum_depth {
        1..=4 => generate_sewer_world(seed, maximum_depth).map_err(MainWorldError::Sewer),
        5 => generate_sewer_world(seed, 4).map_err(MainWorldError::Sewer),
        6..=9 => generate_prison_world(seed, maximum_depth).map_err(MainWorldError::Prison),
        10 => generate_prison_world(seed, 9).map_err(MainWorldError::Prison),
        11..=14 => generate_caves_world(seed, maximum_depth).map_err(MainWorldError::Caves),
        15 => generate_caves_world(seed, 14).map_err(MainWorldError::Caves),
        16..=19 => generate_city_world(seed, maximum_depth).map_err(MainWorldError::City),
        20 => {
            let mut world = generate_halls_world(seed, 21).map_err(MainWorldError::Halls)?;
            world.items.retain(|item| item.depth <= 20);
            Ok(world)
        }
        21..=24 => generate_halls_world(seed, maximum_depth).map_err(MainWorldError::Halls),
        _ => Err(MainWorldError::InvalidMaximumDepth(maximum_depth)),
    }
}

/// Failure while producing a canonical main-dungeon prefix.
#[derive(Clone, Debug, PartialEq)]
pub enum MainWorldError {
    InvalidMaximumDepth(u8),
    Sewer(SewerFloorError),
    Prison(PrisonFloorError),
    Caves(CavesFloorError),
    City(CityFloorError),
    Halls(HallsFloorError),
}

impl fmt::Display for MainWorldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMaximumDepth(depth) => {
                write!(formatter, "main-world depth must be in 1..=24, got {depth}")
            }
            Self::Sewer(error) => error.fmt(formatter),
            Self::Prison(error) => error.fmt(formatter),
            Self::Caves(error) => error.fmt(formatter),
            Self::City(error) => error.fmt(formatter),
            Self::Halls(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for MainWorldError {}

#[cfg(test)]
mod tests {
    use crate::catalog::{ItemId, ItemKind};
    use crate::model::ItemSource;
    use crate::query::{Requirement, SearchQuery};
    use crate::search::WorldGenerator;
    use crate::seed::DungeonSeed;

    use super::{CanonicalMainWorldGenerator, MainWorldError, generate_main_world};

    #[test]
    fn rejects_depths_outside_the_main_searchable_dungeon() {
        let seed = DungeonSeed::MIN;
        assert_eq!(
            generate_main_world(seed, 0),
            Err(MainWorldError::InvalidMaximumDepth(0))
        );
        assert_eq!(
            generate_main_world(seed, 25),
            Err(MainWorldError::InvalidMaximumDepth(25))
        );
    }

    #[test]
    fn state_neutral_boss_depths_match_their_preceding_regular_prefixes() {
        let seed = DungeonSeed::MIN;
        for (boss, regular) in [(5, 4), (10, 9), (15, 14)] {
            assert_eq!(
                generate_main_world(seed, boss).unwrap(),
                generate_main_world(seed, regular).unwrap()
            );
        }
    }

    #[test]
    fn depth_twenty_includes_official_imp_shop_stock_without_halls_items() {
        let world = generate_main_world(DungeonSeed::MIN, 20).unwrap();
        assert!(world.items.iter().all(|item| item.depth <= 20));
        let depth_twenty = world
            .items
            .iter()
            .filter(|item| item.depth == 20)
            .map(|item| item.item)
            .collect::<Vec<_>>();
        assert_eq!(
            depth_twenty,
            vec![
                ItemId::PlateArmor,
                ItemId::ThrowingHammer,
                ItemId::Greatshield,
                ItemId::IncendiaryDart,
            ]
        );
    }

    #[test]
    fn plus_four_imp_ring_is_present_and_searchable() {
        let seed = DungeonSeed::from_code("AAA-AAA-AAF").unwrap();
        let world = generate_main_world(seed, 24).unwrap();
        assert!(world.items.iter().any(|value| {
            value.item == ItemId::RingSharpshooting
                && value.upgrade == 4
                && value.depth == 17
                && value.cursed
                && value.source == ItemSource::ImpReward
        }));
        let query = SearchQuery {
            requirements: vec![Requirement {
                kind: ItemKind::Ring,
                item: Some(ItemId::RingSharpshooting),
                upgrade: Some(4),
                effect: None,
            }],
            max_depth: 24,
        };
        assert_eq!(query.validate(), Ok(()));
        assert!(query.matches(&world));
    }

    #[test]
    fn canonical_depth_twenty_four_batch_matches_scalar_generation() {
        let seeds = [
            DungeonSeed::MIN,
            DungeonSeed::new(1).unwrap(),
            DungeonSeed::new(2).unwrap(),
            DungeonSeed::new(3).unwrap(),
            DungeonSeed::new(26).unwrap(),
            DungeonSeed::from_code("ABC-DEF-GHI").unwrap(),
            DungeonSeed::new(DungeonSeed::MAX.value() - 1).unwrap(),
            DungeonSeed::MAX,
        ];
        let scalar = seeds
            .iter()
            .copied()
            .map(|seed| CanonicalMainWorldGenerator.generate(seed, 24))
            .collect::<Vec<_>>();
        assert_eq!(
            CanonicalMainWorldGenerator.generate_batch(&seeds, 24),
            scalar
        );
    }
}
