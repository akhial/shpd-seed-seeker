//! Native benchmark adapter; uses the macOS build’s core and mimalloc allocator.
//! Usage: `match_benchmark QUERY_JSON WORKERS` (JSON-lines batches on stdin).
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::{AutoTrinketPolicy, search_batch},
    catalog::{item, item_by_stable_id},
    feasibility::QueryPlan,
    json_query,
    main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket},
    query::scout_matches,
    seed::DungeonSeed,
};
use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[allow(clippy::too_many_lines)] // Keep the small adapter protocol and timing boundary together.
fn main() {
    assert!(shpd_seedfinder_ffi::seedfinder_available_workers() > 0);
    let args: Vec<_> = std::env::args().collect();
    let query = json_query::decode(&args[1]).expect("valid benchmark query");
    let workers: usize = args[2].parse().expect("worker count");
    assert!(workers > 0);
    let began = Instant::now();
    let plan = QueryPlan::analyze(&query);
    assert!(!plan.is_unsatisfiable());
    let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
    let setup_seconds = began.elapsed().as_secs_f64();
    let link = shpd_seedfinder_core::deep_link::encode(&query).unwrap();
    let policy = AutoTrinketPolicy::prepare(&query);
    let preferred: Vec<_> = policy
        .iter()
        .flat_map(AutoTrinketPolicy::preferred)
        .map(|id| item(*id).stable_id)
        .collect();
    println!(
        "{}",
        json!({"ready":true,"setup_seconds":setup_seconds,"link":link,"preferred":preferred})
    );
    let mut output = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let seeds: Vec<_> = request["seeds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| DungeonSeed::new(value.as_u64().unwrap()).unwrap())
            .collect();
        if request["verify"].as_bool().unwrap_or(false) {
            let choices = request["trinkets"].as_array().unwrap();
            let mut results = Vec::new();
            for (seed, choice) in seeds.iter().zip(choices) {
                let trinket = choice.as_str().map(|s| item_by_stable_id(s).unwrap().id);
                let world =
                    generate_main_world_with_trinket(*seed, 24, query.challenges, trinket).unwrap();
                results.push(json!({"seed":seed.value(),"verified":query.matches(&world)}));
            }
            writeln!(
                output,
                "{}",
                json!({"tested":seeds.len(),"matches":results})
            )
            .unwrap();
            output.flush().unwrap();
            continue;
        }
        let began = Instant::now();
        let next = AtomicUsize::new(0);
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    let query = &query;
                    let plan = &plan;
                    let generator = &generator;
                    let seeds = &seeds;
                    let next = &next;
                    scope.spawn(move || {
                        // Shared batches keep the M4's performance and efficiency
                        // cores busy without changing production search semantics.
                        let mut matches = Vec::new();
                        loop {
                            let start = next.fetch_add(256, Ordering::Relaxed);
                            if start >= seeds.len() {
                                break;
                            }
                            let batch = &seeds[start..(start + 256).min(seeds.len())];
                            matches.extend(
                                search_batch(generator, query, plan, batch)
                                    .into_iter()
                                    .flatten()
                                    .map(|result| {
                                        let selected = scout_matches(&result.world, query);
                                        assert_eq!(
                                            selected.matched_requirements,
                                            selected.total_requirements
                                        );
                                        let witnesses: Vec<_> = selected
                                            .matched_indices()
                                            .into_iter()
                                            .map(|index| &result.world.items[index])
                                            .map(|candidate| {
                                                json!([
                                                    candidate.depth,
                                                    format!("{:?}", candidate.source),
                                                    item(candidate.item).stable_id,
                                                    candidate.upgrade,
                                                    candidate.cursed,
                                                    candidate
                                                        .effect
                                                        .map_or("-", |effect| effect.wire_name())
                                                ])
                                            })
                                            .collect();
                                        assert!(
                                            !witnesses.is_empty(),
                                            "a reported match needs an item witness"
                                        );
                                        json!({"seed":result.recipe.seed.value(),
                                "trinket":result.recipe.trinket.map(|id|item(id).stable_id),
                                "witnesses":witnesses})
                                    }),
                            );
                        }
                        matches
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        writeln!(
            output,
            "{}",
            json!({"tested":seeds.len(),"seconds":began.elapsed().as_secs_f64(),"matches":results})
        )
        .unwrap();
        output.flush().unwrap();
    }
}
