//! Optional search expansion across independently applied initial trinket offers.
//!
//! Frontends pass their performance setting to [`search_batch`]. The engine
//! owns eligibility, branch generation, pruning, matching, and deduplication.
use crate::catalog::{ItemId, ItemKind};
use crate::feasibility::QueryPlan;
use crate::model::{GeneratedWorld, WorldItem};
use crate::query::SearchQuery;
use crate::quests::QuestSummary;
use crate::search::{FloorGate, WorldGenerator};
use crate::seed::DungeonSeed;
use crate::trinkets::{INITIAL_OFFER_COUNT, trinket_order};

/// A matching world and the optional trinket needed to reproduce it.
#[derive(Clone, Debug)]
pub struct TrinketSearchMatch {
    pub world: GeneratedWorld,
    pub selected_trinket: Option<ItemId>,
}

/// Explicit trinket requirements, including OR alternatives, disable expansion.
#[must_use]
pub fn enabled(query: &SearchQuery, requested: bool) -> bool {
    requested
        && !query
            .requirements
            .iter()
            .any(|r| r.kind == ItemKind::Trinket)
}

struct BranchGate<'a> {
    plan: &'a QueryPlan,
    trinket: ItemId,
}

impl FloorGate for BranchGate<'_> {
    fn selected_trinket(&self, _: DungeonSeed) -> Option<ItemId> {
        Some(self.trinket)
    }

    fn continue_after_floor(&self, depth: u8, items: &[WorldItem], quests: &QuestSummary) -> bool {
        self.plan.continue_after_floor(depth, items, quests)
    }

    fn wants_vault_treasure(&self) -> bool {
        self.plan.wants_vault_treasure()
    }
}

/// Searches a batch with one output per input seed. Baseline matches take
/// precedence; otherwise each applicable initial offer is tried independently
/// until a matching world is found. Items from different branches never mix.
///
/// The caller supplies a validated query and its corresponding plan. The
/// canonical generator activates +3 effects only after the catalyst and an
/// alchemy pot are available, preserving pre-brew floors and challenge rules.
/// The returned recipe can be passed to `generate_main_world_with_trinket`
/// for full scouting. Counts and result limits should count input seeds,
/// rather than the number of attempted branches.
#[must_use]
pub fn search_batch<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    plan: &QueryPlan,
    seeds: &[DungeonSeed],
    auto_apply: bool,
) -> Vec<Option<TrinketSearchMatch>> {
    if plan.is_unsatisfiable() {
        return seeds.iter().map(|_| None).collect();
    }
    let auto_apply = enabled(query, auto_apply);
    generator
        .generate_batch_gated(seeds, plan.generation_depth(), plan)
        .into_iter()
        .zip(seeds)
        .map(|(world, &seed)| {
            if let Some(world) = world.filter(|world| query.matches(world)) {
                return Some(TrinketSearchMatch {
                    world,
                    selected_trinket: plan.selected_trinket(seed),
                });
            }
            if !auto_apply {
                return None;
            }
            trinket_order(seed)[..INITIAL_OFFER_COUNT]
                .iter()
                .copied()
                .filter(|id| {
                    matches!(
                        id,
                        ItemId::ParchmentScrap
                            | ItemId::MimicTooth
                            | ItemId::RatSkull
                            | ItemId::CrackedSpyglass
                            | ItemId::MossyClump
                            | ItemId::TrapMechanism
                    )
                })
                .find_map(|trinket| {
                    generator
                        .generate_batch_gated(
                            &[seed],
                            plan.generation_depth(),
                            &BranchGate { plan, trinket },
                        )
                        .into_iter()
                        .flatten()
                        .find(|world| query.matches(world))
                        .map(|world| TrinketSearchMatch {
                            world,
                            selected_trinket: Some(trinket),
                        })
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::item;
    use crate::challenges::Challenges;
    use crate::main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket};
    use crate::model::{Accessibility, ItemSource};
    use std::sync::Mutex;

    struct BranchFixture {
        template: GeneratedWorld,
        calls: Mutex<Vec<Option<ItemId>>>,
    }

    impl WorldGenerator for BranchFixture {
        fn generate(&self, _: DungeonSeed, _: u8) -> GeneratedWorld {
            self.template.clone()
        }

        fn generate_batch_gated(
            &self,
            seeds: &[DungeonSeed],
            _: u8,
            gate: &dyn FloorGate,
        ) -> Vec<Option<GeneratedWorld>> {
            seeds
                .iter()
                .map(|&seed| {
                    let selected = gate.selected_trinket(seed);
                    self.calls.lock().unwrap().push(selected);
                    // A pruned baseline must not prevent trying a later branch.
                    let id = match selected {
                        Some(ItemId::MimicTooth) => ItemId::RingHaste,
                        Some(ItemId::ParchmentScrap) => ItemId::RingMight,
                        _ => return None,
                    };
                    let mut world = self.template.clone();
                    world.items = vec![WorldItem {
                        item: id,
                        upgrade: 2,
                        effect: None,
                        cursed: false,
                        depth: 6,
                        source: ItemSource::Heap,
                        accessibility: Accessibility::Independent,
                        secret: false,
                    }];
                    Some(world)
                })
                .collect()
        }
    }

    #[test]
    fn tries_later_offers_without_combining_incompatible_branches() {
        let generator = BranchFixture {
            template: CanonicalMainWorldGenerator.generate(DungeonSeed::MIN, 6),
            calls: Mutex::default(),
        };
        let query = crate::json_query::decode(
            r#"{"max_depth":6,"requirements":[{"item":"ring_might","upgrade":2}]}"#,
        )
        .unwrap();
        let results = search_batch(
            &generator,
            &query,
            &QueryPlan::analyze(&query),
            &[DungeonSeed::MIN],
            true,
        );
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].as_ref().unwrap().selected_trinket,
            Some(ItemId::ParchmentScrap)
        );
        let calls = generator.calls.lock().unwrap().clone();
        assert_eq!(calls[0], None);
        assert!(calls.contains(&Some(ItemId::MimicTooth)));
        assert!(calls.contains(&Some(ItemId::ParchmentScrap)));
        let query = crate::json_query::decode(
            r#"{"max_depth":6,"requirements":[{"item":"ring_might"},{"item":"ring_haste"}]}"#,
        )
        .unwrap();
        assert!(
            search_batch(
                &generator,
                &query,
                &QueryPlan::analyze(&query),
                &[DungeonSeed::MIN],
                true
            )[0]
            .is_none()
        );
    }

    #[test]
    fn feeling_trinkets_recover_searchable_items_baseline_misses() {
        for (value, trinket, document) in [
            (
                2,
                ItemId::MossyClump,
                r#"{"max_depth":6,"requirements":[{"item":"cleansing_dart","uncursed":true}]}"#,
            ),
            (
                5,
                ItemId::TrapMechanism,
                r#"{"max_depth":7,"requirements":[{"item":"spear","upgrade":1,"effect":"Explosive"}]}"#,
            ),
        ] {
            let seed = DungeonSeed::new(value).unwrap();
            let query = crate::json_query::decode(document).unwrap();
            let plan = QueryPlan::analyze(&query);
            assert!(trinket_order(seed)[..INITIAL_OFFER_COUNT].contains(&trinket));
            assert!(
                search_batch(&CanonicalMainWorldGenerator, &query, &plan, &[seed], false)[0]
                    .is_none()
            );
            let world = generate_main_world_with_trinket(seed, 24, Challenges::NONE, Some(trinket))
                .unwrap();
            assert!(query.matches(&world));
            let result = search_batch(&CanonicalMainWorldGenerator, &query, &plan, &[seed], true);
            assert_eq!(result[0].as_ref().unwrap().selected_trinket, Some(trinket));
        }
    }

    #[test]
    fn real_matches_reproduce_under_challenges_and_preserve_baseline() {
        let mut query = crate::json_query::decode(
            r#"{"max_depth":9,"requirements":[{"kind":"ring","upgrade":2}]}"#,
        )
        .unwrap();
        query.challenges = Challenges::NO_FOOD | Challenges::DARKNESS;
        let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
        let seeds = (0..24)
            .map(|v| DungeonSeed::new(v).unwrap())
            .collect::<Vec<_>>();
        let plan = QueryPlan::analyze(&query);
        let baseline = search_batch(&generator, &query, &plan, &seeds, false);
        let expanded = search_batch(&generator, &query, &plan, &seeds, true);
        for ((base, found), seed) in baseline.iter().zip(&expanded).zip(seeds) {
            if base.is_some() {
                assert!(found.is_some());
                assert!(found.as_ref().unwrap().selected_trinket.is_none());
            }
            if let Some(found) = found {
                if let Some(id) = found.selected_trinket {
                    assert_eq!(item(id).kind, ItemKind::Trinket);
                    assert!(trinket_order(seed)[..INITIAL_OFFER_COUNT].contains(&id));
                }
                let scouted = generate_main_world_with_trinket(
                    seed,
                    24,
                    query.challenges,
                    found.selected_trinket,
                )
                .unwrap();
                assert!(query.matches(&scouted));
            }
        }
    }
}
