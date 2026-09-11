//! Query-aware selection of one initial trinket before world generation.
//!
//! Preparing a policy uses the measured equipment profiles once per search.
//! Resolving it only reads the seed's private offer deck: it never scouts or
//! branches, and never combines loot from different possible worlds.
use crate::catalog::{Effect, ItemId, ItemKind};
use crate::feasibility::QueryPlan;
use crate::model::{GeneratedWorld, WorldItem};
use crate::probability::equipment_probability;
use crate::probability_tables::trinkets::Profile;
use crate::query::{EffectRequirement, SearchQuery};
use crate::quests::QuestSummary;
use crate::search::{FloorGate, WorldGenerator};
use crate::seed::DungeonSeed;
use crate::trinkets::{INITIAL_OFFER_COUNT, trinket_order};

/// Reproducible world conditions for a result, independent of editor changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeedRecipe {
    pub seed: DungeonSeed,
    pub trinket: Option<ItemId>,
}

/// A matching world with the exact choice used to generate it.
#[derive(Clone, Debug)]
pub struct TrinketSearchMatch {
    pub world: GeneratedWorld,
    pub recipe: SeedRecipe,
}

/// Search once per input seed using a prepared query plan. Auto-applied
/// matches alone get a no-trinket replay to remove unnecessary choices.
/// One output per input preserves traversal accounting even for pruned seeds.
#[must_use]
pub fn search_batch<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    plan: &QueryPlan,
    seeds: &[DungeonSeed],
) -> Vec<Option<TrinketSearchMatch>> {
    if plan.is_unsatisfiable() {
        return seeds.iter().map(|_| None).collect();
    }
    let results = match_batch(generator, query, plan, plan.generation_depth(), seeds);
    remove_unnecessary_trinkets(generator, query, plan, results)
}

struct RecipeGate<'a> {
    plan: &'a QueryPlan,
    choices: std::collections::BTreeMap<u64, Option<ItemId>>,
}

impl FloorGate for RecipeGate<'_> {
    fn selected_trinket(&self, seed: DungeonSeed) -> Option<ItemId> {
        self.choices.get(&seed.value()).copied().flatten()
    }
    fn continue_after_floor(&self, depth: u8, items: &[WorldItem], quests: &QuestSummary) -> bool {
        self.plan.continue_after_floor(depth, items, quests)
    }
    fn wants_vault_treasure(&self) -> bool {
        self.plan.wants_vault_treasure()
    }
}

fn remove_unnecessary_trinkets<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    plan: &QueryPlan,
    mut results: Vec<Option<TrinketSearchMatch>>,
) -> Vec<Option<TrinketSearchMatch>> {
    if !enabled(query) {
        return results;
    }
    let selected: Vec<_> = results
        .iter()
        .enumerate()
        .filter_map(|(index, result)| {
            result
                .as_ref()
                .and_then(|result| result.recipe.trinket.map(|_| (index, result.recipe.seed)))
        })
        .collect();
    if selected.is_empty() {
        return results;
    }
    // Reuse the query's floor/vault pruning without its automatic choice.
    // Failed initial searches never get this extra generation pass.
    let gate = RecipeGate {
        plan,
        choices: std::collections::BTreeMap::new(),
    };
    let seeds: Vec<_> = selected.iter().map(|&(_, seed)| seed).collect();
    let baseline = match_batch(generator, query, &gate, plan.generation_depth(), &seeds);
    for ((index, _), result) in selected.into_iter().zip(baseline) {
        if result.is_some() {
            // Replace the world as well as its recipe: callers must see the
            // exact no-trinket items, not loot from the original generation.
            results[index] = result;
        }
    }
    results
}

/// Verify saved recipes against new item predicates while retaining their
/// original world conditions, then remove any unnecessary automatic choice.
#[must_use]
pub fn filter_batch<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    plan: &QueryPlan,
    recipes: &[SeedRecipe],
) -> Vec<Option<TrinketSearchMatch>> {
    if plan.is_unsatisfiable() {
        return recipes.iter().map(|_| None).collect();
    }
    let gate = RecipeGate {
        plan,
        choices: recipes
            .iter()
            .map(|r| (r.seed.value(), r.trinket))
            .collect(),
    };
    let seeds: Vec<_> = recipes.iter().map(|r| r.seed).collect();
    let results = match_batch(generator, query, &gate, plan.generation_depth(), &seeds);
    remove_unnecessary_trinkets(generator, query, plan, results)
}

/// Refine saved results under a new query. An automatic choice removed for
/// the base query may be necessary for the new one, so retry the policy's
/// choice when a saved no-trinket recipe fails changed predicates.
/// Callers use `SearchQuery::continues` to decide whether
/// they can also reuse the base query's scanned coverage.
#[must_use]
pub fn refine_batch<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    plan: &QueryPlan,
    base: &SearchQuery,
    recipes: &[SeedRecipe],
) -> Vec<Option<TrinketSearchMatch>> {
    let mut results = filter_batch(generator, query, plan, recipes);
    if !enabled(query) || query == base || plan.is_unsatisfiable() {
        return results;
    }
    let retry: Vec<_> = recipes
        .iter()
        .enumerate()
        .filter_map(|(index, recipe)| {
            (results[index].is_none()
                && recipe.trinket.is_none()
                && plan.selected_trinket(recipe.seed).is_some())
            .then_some((index, recipe.seed))
        })
        .collect();
    if !retry.is_empty() {
        let seeds: Vec<_> = retry.iter().map(|&(_, seed)| seed).collect();
        // The baseline just failed, so any match here needs its trinket.
        // Saved recipes that still match retain their verified world.
        let recovered = match_batch(generator, query, plan, plan.generation_depth(), &seeds);
        for ((index, _), result) in retry.into_iter().zip(recovered) {
            results[index] = result;
        }
    }
    results
}

fn match_batch<G: WorldGenerator>(
    generator: &G,
    query: &SearchQuery,
    gate: &dyn FloorGate,
    depth: u8,
    seeds: &[DungeonSeed],
) -> Vec<Option<TrinketSearchMatch>> {
    // The caller's prepared gate owns pruning. QueryPlan also selects the
    // minimal generation horizon; recipe overrides do not change sources.
    generator
        .generate_batch_gated(seeds, depth, gate)
        .into_iter()
        .map(|world| {
            world
                .filter(|world| query.matches(world))
                .map(|world| TrinketSearchMatch {
                    recipe: SeedRecipe {
                        seed: world.seed,
                        trinket: gate.selected_trinket(world.seed),
                    },
                    world,
                })
        })
        .collect()
}

/// Generation effects with a directional equipment benefit. Feeling changes
/// and neutral trinkets are deliberately excluded.
pub const CANDIDATES: [ItemId; 4] = [
    ItemId::ParchmentScrap,
    ItemId::MimicTooth,
    ItemId::RatSkull,
    ItemId::CrackedSpyglass,
];

/// The complete deterministic choice rule. Equality means identical choices
/// for every offer deck, which is needed for safe filter-and-resume searches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoTrinketPolicy {
    preferred: Vec<ItemId>,
}

impl AutoTrinketPolicy {
    /// Prepare once per query. Any trinket requirement, even inside an OR
    /// group, disables automatic selection in favor of the explicit rules.
    #[must_use]
    pub fn prepare(query: &SearchQuery) -> Option<Self> {
        if !enabled(query) {
            return None;
        }
        let baseline = equipment_probability(query, Profile::None);
        let forbids_parchment = query.requirements.iter().any(|r| match r.effect {
            EffectRequirement::OneOf(set) => set.effects().any(Effect::is_curse),
            EffectRequirement::Any => false,
        });
        let mut scores: Vec<_> = CANDIDATES
            .into_iter()
            .filter(|&id| id != ItemId::ParchmentScrap || !forbids_parchment)
            .map(|id| (id, equipment_probability(query, Profile::of(id))))
            .collect();
        // Stable sorting gives a fixed tie-break, independent of offer order.
        scores.sort_by(|a, b| finite_score(b.1).total_cmp(&finite_score(a.1)));
        Some(Self {
            preferred: scores
                .iter()
                .filter(|(_, p)| baseline.is_finite() && p.is_finite() && *p > baseline * 1.05)
                .map(|&(id, _)| id)
                .collect(),
        })
    }

    /// Choose a beneficial initial offer, or none, without generating floors.
    #[must_use]
    pub fn selected_trinket(&self, seed: DungeonSeed) -> Option<ItemId> {
        if self.preferred.is_empty() {
            return None;
        }
        self.choose(&trinket_order(seed)[..INITIAL_OFFER_COUNT])
    }

    pub(crate) fn choose(&self, offers: &[ItemId]) -> Option<ItemId> {
        self.preferred
            .iter()
            .copied()
            .find(|id| offers.contains(id))
    }

    /// Preferred generation effects, in descending estimated match probability.
    #[must_use]
    pub fn preferred(&self) -> &[ItemId] {
        &self.preferred
    }
}

fn finite_score(score: f64) -> f64 {
    if score.is_finite() { score } else { 0.0 }
}

/// Whether the requested setting applies to this query.
#[must_use]
pub fn enabled(query: &SearchQuery) -> bool {
    query.auto_apply_trinket
        && !query
            .requirements
            .iter()
            .any(|r| r.kind == ItemKind::Trinket)
}

/// Queries can share scanned coverage only when their choice rules agree.
#[must_use]
pub fn same_selection(candidate: &SearchQuery, base: &SearchQuery) -> bool {
    candidate == base || AutoTrinketPolicy::prepare(candidate) == AutoTrinketPolicy::prepare(base)
}

/// Probability of the chosen policy, averaged over all 2,380 offer subsets.
/// Equipment profiles already include the first brewing opportunity.
pub(crate) fn probability(query: &SearchQuery, policy: &AutoTrinketPolicy) -> f64 {
    let baseline = equipment_probability(query, Profile::None);
    let scores = CANDIDATES.map(|id| equipment_probability(query, Profile::of(id)));
    let identities = trinket_order(DungeonSeed::MIN);
    let mut sum = 0.0;
    let mut count = 0_u32;
    for a in 0..14 {
        for b in a + 1..15 {
            for c in b + 1..16 {
                for d in c + 1..17 {
                    let selected = policy.choose(&[
                        identities[a],
                        identities[b],
                        identities[c],
                        identities[d],
                    ]);
                    sum += selected
                        .and_then(|id| CANDIDATES.iter().position(|&candidate| candidate == id))
                        .map_or(baseline, |index| scores[index]);
                    count += 1;
                }
            }
        }
    }
    sum / f64::from(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::challenges::Challenges;
    use crate::json_query;
    use crate::main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket};
    use crate::query::{StartDecision, decide_start};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn query(requirements: &str) -> SearchQuery {
        json_query::decode(&format!(
            r#"{{"auto_apply_trinket":true,"max_depth":19,"requirements":{requirements}}}"#
        ))
        .unwrap()
    }

    struct CountingGenerator(AtomicUsize);
    impl WorldGenerator for CountingGenerator {
        fn generate(&self, _: DungeonSeed, _: u8) -> GeneratedWorld {
            panic!("batch path expected")
        }
        fn generate_batch_gated(
            &self,
            seeds: &[DungeonSeed],
            depth: u8,
            gate: &dyn FloorGate,
        ) -> Vec<Option<GeneratedWorld>> {
            self.0.fetch_add(seeds.len(), Ordering::Relaxed);
            CanonicalMainWorldGenerator.generate_batch_gated(seeds, depth, gate)
        }
    }

    #[test]
    fn every_offer_set_selects_only_a_beneficial_choice_or_none() {
        let identities = trinket_order(DungeonSeed::MIN);
        for requirements in [
            r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#,
            r#"[{"kind":"melee_weapon","effect":["Grim","Annoying"]}]"#,
        ] {
            let policy = AutoTrinketPolicy::prepare(&query(requirements)).unwrap();
            let curse = requirements.contains("Annoying");
            for a in 0..14 {
                for b in a + 1..15 {
                    for c in b + 1..16 {
                        for d in c + 1..17 {
                            let offers =
                                [identities[a], identities[b], identities[c], identities[d]];
                            if let Some(choice) = policy.choose(&offers) {
                                assert!(offers.contains(&choice));
                                assert!(policy.preferred().contains(&choice));
                                assert!(!curse || choice != ItemId::ParchmentScrap);
                            } else {
                                assert!(offers.iter().all(|id| !policy.preferred().contains(id)));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unhelpful_offers_keep_the_no_trinket_world() {
        let mut early = query(r#"[{"item":"leather_armor","upgrade":1}]"#);
        early.max_depth = 2;
        let policy = AutoTrinketPolicy::prepare(&early).unwrap();
        assert!(policy.preferred().is_empty());
        assert_eq!(policy.selected_trinket(DungeonSeed::MIN), None);
        let mut baseline = early.clone();
        baseline.auto_apply_trinket = false;
        let seeds: Vec<_> = (0..64)
            .map(|value| DungeonSeed::new(value).unwrap())
            .collect();
        let automatic = search_batch(
            &CanonicalMainWorldGenerator,
            &early,
            &QueryPlan::analyze(&early),
            &seeds,
        );
        let plain = search_batch(
            &CanonicalMainWorldGenerator,
            &baseline,
            &QueryPlan::analyze(&baseline),
            &seeds,
        );
        assert!(automatic.iter().any(Option::is_some));
        for (automatic, plain) in automatic.into_iter().zip(plain) {
            assert_eq!(
                automatic.as_ref().map(|m| &m.world),
                plain.as_ref().map(|m| &m.world)
            );
            if let Some(result) = automatic {
                assert_eq!(result.recipe.trinket, None);
                assert!(baseline.matches(&result.world));
            }
        }
        let grim = AutoTrinketPolicy::prepare(&query(
            r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#,
        ))
        .unwrap();
        assert_eq!(
            grim.choose(&[
                ItemId::SaltCube,
                ItemId::WondrousResin,
                ItemId::MossyClump,
                ItemId::TrapMechanism
            ]),
            None
        );
    }

    #[test]
    fn whole_query_ranking_and_explicit_requirements_control_selection() {
        let grim = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#);
        assert_eq!(
            AutoTrinketPolicy::prepare(&grim).unwrap().preferred()[0],
            ItemId::ParchmentScrap
        );
        let ring = query(r#"[{"item":"ring_might","upgrade":2,"max_depth":9}]"#);
        assert_eq!(
            AutoTrinketPolicy::prepare(&ring).unwrap().preferred()[0],
            ItemId::MimicTooth
        );
        let explicit = query(
            r#"[{"any_of":[{"item":"mimic_tooth","select_trinket":true},{"item":"rat_skull"}]}]"#,
        );
        assert!(AutoTrinketPolicy::prepare(&explicit).is_none());
        let unselected = query(r#"[{"item":"rat_skull"}]"#);
        assert!(AutoTrinketPolicy::prepare(&unselected).is_none());
        let mut baseline = grim.clone();
        baseline.auto_apply_trinket = false;
        assert!(AutoTrinketPolicy::prepare(&baseline).is_none());
        assert!(
            crate::probability::estimate_match_probability(&grim)
                > crate::probability::estimate_match_probability(&baseline)
        );
    }

    #[test]
    fn changed_world_choices_start_a_new_traversal() {
        let base = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#);
        assert!(base.continues(&base));
        assert_eq!(
            decide_start(&base, Some(&base), false, true, None),
            StartDecision::TargetRefine
        );
        let mut disabled = base.clone();
        disabled.auto_apply_trinket = false;
        assert!(!disabled.continues(&base));
        assert_eq!(
            decide_start(&disabled, Some(&base), false, true, None),
            StartDecision::Detached
        );
        let cursed = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Annoying"}]"#);
        assert_eq!(
            decide_start(&cursed, Some(&base), false, true, None),
            StartDecision::Detached
        );
        let explicit = query(r#"[{"item":"rat_skull"}]"#);
        let mut explicit_off = explicit.clone();
        explicit_off.auto_apply_trinket = false;
        assert!(explicit.continues(&explicit_off));
    }

    #[test]
    fn necessary_trinkets_survive_baseline_replay_and_saved_choices_replay_the_match() {
        let seed = DungeonSeed::from_code("SRU-YSU-QHS").unwrap();
        let query = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#);
        let plan = QueryPlan::analyze(&query);
        let generator = CountingGenerator(AtomicUsize::new(0));
        let result = search_batch(&generator, &query, &plan, &[seed])
            .pop()
            .unwrap()
            .unwrap();
        assert_eq!(generator.0.load(Ordering::Relaxed), 2);
        assert_eq!(result.recipe.trinket, Some(ItemId::ParchmentScrap));
        let baseline = generate_main_world_with_trinket(seed, 24, Challenges::NONE, None).unwrap();
        assert!(!query.matches(&baseline));
        let replay =
            generate_main_world_with_trinket(seed, 24, Challenges::NONE, result.recipe.trinket)
                .unwrap();
        assert!(query.matches(&replay));
        assert_eq!(
            baseline
                .items
                .iter()
                .filter(|i| i.depth <= 2)
                .collect::<Vec<_>>(),
            replay
                .items
                .iter()
                .filter(|i| i.depth <= 2)
                .collect::<Vec<_>>()
        );
        assert!(filter_batch(&generator, &query, &plan, &[result.recipe])[0].is_some());
        assert!(
            filter_batch(
                &generator,
                &query,
                &plan,
                &[SeedRecipe {
                    seed,
                    trinket: None
                }]
            )[0]
            .is_none()
        );
        assert_eq!(generator.0.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn only_auto_applied_matches_are_rechecked_and_unneeded_worlds_are_replaced() {
        let seed = DungeonSeed::from_code("EYY-RUL-LQG").unwrap();
        let query = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#);
        let plan = QueryPlan::analyze(&query);
        assert_eq!(plan.selected_trinket(seed), Some(ItemId::ParchmentScrap));
        let generator = CountingGenerator(AtomicUsize::new(0));
        let required = DungeonSeed::from_code("SRU-YSU-QHS").unwrap();
        let results = search_batch(
            &generator,
            &query,
            &plan,
            &[seed, DungeonSeed::MIN, required],
        );
        assert_eq!(results.len(), 3);
        assert!(results[1].is_none());
        assert_eq!(
            results[2].as_ref().unwrap().recipe.trinket,
            Some(ItemId::ParchmentScrap)
        );
        assert_eq!(generator.0.load(Ordering::Relaxed), 5); // Three searches, two match rechecks.
        let result = results[0].as_ref().unwrap();
        assert_eq!(
            result.recipe,
            SeedRecipe {
                seed,
                trinket: None
            }
        );

        let mut baseline_query = query.clone();
        baseline_query.auto_apply_trinket = false;
        let baseline = search_batch(
            &generator,
            &baseline_query,
            &QueryPlan::analyze(&baseline_query),
            &[seed],
        );
        assert_eq!(result.world, baseline[0].as_ref().unwrap().world);
        assert_eq!(generator.0.load(Ordering::Relaxed), 6); // No recheck with auto off.
        let filtered = filter_batch(&generator, &query, &plan, &[result.recipe]);
        assert_eq!(filtered[0].as_ref().unwrap().world, result.world);
        assert_eq!(generator.0.load(Ordering::Relaxed), 7); // No recheck for a null recipe.

        let explicit = json_query::decode(
            r#"{"auto_apply_trinket":true,"max_depth":19,
            "requirements":[{"item":"runic_blade","upgrade":1,"effect":"Grim"},
                {"item":"parchment_scrap","select_trinket":true}]}"#,
        )
        .unwrap();
        let explicit_result = search_batch(
            &generator,
            &explicit,
            &QueryPlan::analyze(&explicit),
            &[seed],
        );
        assert_eq!(
            explicit_result[0].as_ref().unwrap().recipe.trinket,
            Some(ItemId::ParchmentScrap)
        );
        assert_eq!(generator.0.load(Ordering::Relaxed), 8); // Manual choices are preserved.
    }

    #[test]
    fn refinement_can_restore_a_trinket_removed_for_the_base_query() {
        let seed = DungeonSeed::from_code("EYY-RUL-LQG").unwrap();
        let base = query(r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]"#);
        let narrowed = query(
            r#"[{"item":"runic_blade","upgrade":1,"effect":"Grim"},
            {"item":"whip","effect":"Venomous"}]"#,
        );
        assert!(narrowed.continues(&base));
        let generator = CanonicalMainWorldGenerator;
        let base_plan = QueryPlan::analyze(&base);
        let plan = QueryPlan::analyze(&narrowed);
        let original = search_batch(&generator, &base, &base_plan, &[seed]);
        let recipe = original[0].as_ref().unwrap().recipe;
        assert_eq!(recipe.trinket, None);
        assert!(filter_batch(&generator, &narrowed, &plan, &[recipe])[0].is_none());

        let refined = refine_batch(&generator, &narrowed, &plan, &base, &[recipe]);
        let refined = refined[0].as_ref().unwrap();
        assert_eq!(refined.recipe.trinket, Some(ItemId::ParchmentScrap));
        assert!(narrowed.matches(&refined.world));
        let replay =
            generate_main_world_with_trinket(seed, 24, Challenges::NONE, refined.recipe.trinket)
                .unwrap();
        assert!(narrowed.matches(&replay));

        // Refining back removes the choice, including when replaying an old
        // imported result whose trinket was unnecessary for the same query.
        let widened = refine_batch(&generator, &base, &base_plan, &narrowed, &[refined.recipe]);
        assert_eq!(widened[0].as_ref().unwrap().recipe, recipe);
        let imported = refine_batch(&generator, &base, &base_plan, &base, &[refined.recipe]);
        assert_eq!(
            imported[0].as_ref().unwrap().world,
            original[0].as_ref().unwrap().world
        );
    }
}
