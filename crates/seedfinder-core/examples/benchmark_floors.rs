//! Compare identical queries with and without floor pruning. Run in release mode.
use shpd_seedfinder_core::{
    feasibility::QueryPlan,
    json_query,
    main_world::CanonicalMainWorldGenerator,
    model::{GeneratedWorld, WorldItem},
    probability::estimate_match_probability,
    quests::QuestSummary,
    search::{FloorGate, WorldGenerator},
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::time::Instant;

struct WithoutFloorFilters<'a>(&'a QueryPlan);
impl FloorGate for WithoutFloorFilters<'_> {
    fn continue_after_floor(&self, depth: u8, items: &[WorldItem], quests: &QuestSummary) -> bool {
        self.0.continue_after_floor(depth, items, quests)
    }
    fn wants_vault_treasure(&self) -> bool {
        self.0.wants_vault_treasure()
    }
    fn deferred_vault_plan(&self, depth: u8) -> Option<&QueryPlan> {
        self.0.deferred_vault_plan(depth)
    }
}
fn main() {
    let seeds: Vec<_> = (0..4096u64)
        .map(|i| DungeonSeed::new((961_373 + i * 3_355_211_884_971) % TOTAL_SEEDS).unwrap())
        .collect();
    for (name, document) in [
        (
            "Darkness 17",
            r#"{"requirements":[],"floor_requirements":[{"depth":17,"feeling":"dark"}]}"#,
        ),
        (
            "Hidden garden 7",
            r#"{"requirements":[],"floor_requirements":[{"depth":7,"rooms":["secret_garden"]}]}"#,
        ),
        (
            "Farm 7",
            r#"{"requirements":[],"floor_requirements":[{"depth":7,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"#,
        ),
        (
            "Farm 17",
            r#"{"requirements":[],"floor_requirements":[{"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"#,
        ),
        (
            "Farm 22",
            r#"{"requirements":[],"floor_requirements":[{"depth":22,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"#,
        ),
        (
            "RoW by 16 + farm 17",
            r#"{"requirements":[{"item":"ring_wealth","max_depth":16}],"floor_requirements":[{"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}]}"#,
        ),
        (
            "RoW only",
            r#"{"requirements":[{"item":"ring_wealth","max_depth":16}]}"#,
        ),
    ] {
        let query = json_query::decode(document).unwrap();
        let plan = QueryPlan::analyze(&query);
        let collect = |worlds: Vec<Option<GeneratedWorld>>| {
            worlds
                .into_iter()
                .flatten()
                .filter(|world| query.matches(world))
                .map(|world| world.seed)
                .collect::<Vec<_>>()
        };
        let started = Instant::now();
        let expected = collect(CanonicalMainWorldGenerator.generate_batch_gated(
            &seeds,
            plan.generation_depth(),
            &WithoutFloorFilters(&plan),
        ));
        let baseline = started.elapsed();
        let started = Instant::now();
        let actual = collect(CanonicalMainWorldGenerator.generate_batch_gated(
            &seeds,
            plan.generation_depth(),
            &plan,
        ));
        let optimized = started.elapsed();
        assert_eq!(actual, expected);
        println!(
            "{name}: {} matches, {:.2}x, baseline={baseline:?}, optimized={optimized:?}, estimate=1/{:.1}, first={}",
            actual.len(),
            baseline.as_secs_f64() / optimized.as_secs_f64(),
            1.0 / estimate_match_probability(&query),
            actual
                .first()
                .map_or_else(|| "none".into(), ToString::to_string)
        );
    }
}
