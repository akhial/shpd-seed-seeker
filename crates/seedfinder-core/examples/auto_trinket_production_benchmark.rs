//! Paired validation of the production single-choice search API.
//! Build with --release --features json-query, then run COUNT START CASE.
//! Each mode searches every input once; verification runs outside the timers.
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::{AutoTrinketPolicy, search_batch},
    catalog::{ItemId, item},
    feasibility::QueryPlan,
    json_query,
    main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket},
    model::GeneratedWorld,
    search::{FloorGate, PRODUCTION_SEARCH_START_STRIDE, WorldGenerator},
    seed::{DungeonSeed, TOTAL_SEEDS},
    trinkets::{INITIAL_OFFER_COUNT, trinket_order},
};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

struct CountingGenerator<G> {
    inner: G,
    inputs: AtomicU64,
}

impl<G: WorldGenerator> WorldGenerator for CountingGenerator<G> {
    fn generate(&self, seed: DungeonSeed, depth: u8) -> GeneratedWorld {
        self.inputs.fetch_add(1, Ordering::Relaxed);
        self.inner.generate(seed, depth)
    }

    fn generate_batch_gated(
        &self,
        seeds: &[DungeonSeed],
        depth: u8,
        gate: &dyn FloorGate,
    ) -> Vec<Option<GeneratedWorld>> {
        self.inputs.fetch_add(seeds.len() as u64, Ordering::Relaxed);
        self.inner.generate_batch_gated(seeds, depth, gate)
    }
}

fn seed_at(index: u64) -> DungeonSeed {
    DungeonSeed::new(
        u64::try_from(
            (u128::from(index) * u128::from(PRODUCTION_SEARCH_START_STRIDE) + 812_345_678_901)
                % u128::from(TOTAL_SEEDS),
        )
        .expect("modulo a u64 seed count fits u64"),
    )
    .unwrap()
}

fn cases() -> Vec<(&'static str, Value)> {
    vec![
        (
            "grim_runic_blade_plus1_depth19",
            json!({"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]}),
        ),
        (
            "ring_might_plus2_depth9",
            json!({"max_depth":9,"requirements":[{"item":"ring_might","upgrade":2}]}),
        ),
        (
            "annoying_weapon_plus1_depth9",
            json!({"max_depth":9,"requirements":[{"kind":"melee_weapon","upgrade":1,"effect":"Annoying"}]}),
        ),
    ]
}

fn process_ticks() -> u64 {
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
            Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
        })
        .unwrap_or(0)
}

#[allow(clippy::too_many_lines)] // Keep paired timing, validation, and accounting together.
fn bench(name: &str, document: &Value, count: u64, start: u64) {
    let baseline = json_query::decode(&document.to_string()).unwrap();
    let mut automatic = baseline.clone();
    automatic.auto_apply_trinket = true;
    let queries = [&baseline, &automatic];
    let mut setup_seconds = [0.0; 2];
    let plans: Vec<_> = queries
        .iter()
        .enumerate()
        .map(|(mode, query)| {
            let clock = Instant::now();
            let plan = QueryPlan::analyze(query);
            setup_seconds[mode] = clock.elapsed().as_secs_f64();
            assert!(!plan.is_unsatisfiable());
            plan
        })
        .collect();
    let policy = AutoTrinketPolicy::prepare(&automatic).unwrap();
    let generator = CountingGenerator {
        inner: CanonicalMainWorldGenerator::with_challenges(baseline.challenges),
        inputs: AtomicU64::new(0),
    };
    let warm: Vec<_> = (9_000_000..9_000_064).map(seed_at).collect();
    for mode in 0..2 {
        drop(search_batch(&generator, queries[mode], &plans[mode], &warm));
    }
    generator.inputs.store(0, Ordering::Relaxed);
    let ticks_per_second = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let mut seconds = [0.0; 2];
    let mut cpu_ticks = [0_u64; 2];
    let mut matches = [0_u64; 2];
    let mut total_cells = [0_u64; 4];
    let mut selected_cells = BTreeMap::<String, [u64; 4]>::new();
    let mut blocks = Vec::new();
    let mut examples = Vec::new();
    let mut replayed = [0_u64; 2];
    let mut baseline_rechecks = 0_u64;
    let mut removed_trinkets = 0_u64;
    let mut done = 0_u64;
    while done < count {
        let seeds: Vec<_> = (start + done..start + (done + 32).min(count))
            .map(seed_at)
            .collect();
        let mut flags = [vec![false; seeds.len()], vec![false; seeds.len()]];
        let mut block_seconds = [0.0; 2];
        let mut block_ticks = [0_u64; 2];
        let mut recipes = [Vec::new(), Vec::new()];
        for mode in if done % 64 == 0 { [0, 1] } else { [1, 0] } {
            let before = process_ticks();
            let clock = Instant::now();
            let results = search_batch(&generator, queries[mode], &plans[mode], &seeds);
            assert_eq!(results.len(), seeds.len());
            for (i, result) in results.into_iter().enumerate() {
                if let Some(result) = result {
                    if mode == 1 && plans[mode].selected_trinket(result.recipe.seed).is_some() {
                        baseline_rechecks += 1;
                        removed_trinkets += u64::from(result.recipe.trinket.is_none());
                    }
                    flags[mode][i] = true;
                    recipes[mode].push(result.recipe);
                }
            }
            block_seconds[mode] = clock.elapsed().as_secs_f64();
            block_ticks[mode] = process_ticks() - before;
            seconds[mode] += block_seconds[mode];
            cpu_ticks[mode] += block_ticks[mode];
            matches[mode] += recipes[mode].len() as u64;
        }
        // Verify every match using its returned recipe and a full floor-24 scout.
        // This work, deck assertions, and bookkeeping are outside both timers.
        for mode in 0..2 {
            for recipe in &recipes[mode] {
                let replay = generate_main_world_with_trinket(
                    recipe.seed,
                    24,
                    baseline.challenges,
                    recipe.trinket,
                )
                .unwrap();
                assert!(queries[mode].matches(&replay), "recipe failed full scout");
                if recipe.trinket.is_some() {
                    assert_eq!(recipe.trinket, plans[mode].selected_trinket(recipe.seed));
                    let baseline_world = generate_main_world_with_trinket(
                        recipe.seed,
                        24,
                        baseline.challenges,
                        None,
                    )
                    .unwrap();
                    assert!(
                        !queries[mode].matches(&baseline_world),
                        "unnecessary trinket retained"
                    );
                }
                replayed[mode] += 1;
            }
        }
        let mut cells = [0_u64; 4];
        for (i, &seed) in seeds.iter().enumerate() {
            let selected = plans[1].selected_trinket(seed);
            if let Some(selected) = selected {
                assert!(trinket_order(seed)[..INITIAL_OFFER_COUNT].contains(&selected));
                assert!(policy.preferred().contains(&selected));
                if name.starts_with("annoying") {
                    assert_ne!(selected, ItemId::ParchmentScrap);
                }
            }
            let cell = usize::from(flags[0][i]) + 2 * usize::from(flags[1][i]);
            cells[cell] += 1;
            total_cells[cell] += 1;
            selected_cells
                .entry(selected.map_or("none", |id| item(id).stable_id).to_owned())
                .or_default()[cell] += 1;
            if cell == 2 && examples.len() < 3 {
                examples.push(json!({"seed":seed.to_code(),"value":seed.value(),
                    "trinket":selected.map(|id| item(id).stable_id)}));
            }
        }
        blocks.push(
            json!({"n":seeds.len(),"seconds":block_seconds,"cpu_ticks":block_ticks,"cells":cells}),
        );
        done += seeds.len() as u64;
        if done % 8192 == 0 {
            eprintln!("{name}: {done}/{count}, matches {matches:?}, seconds {seconds:?}");
        }
    }
    assert_eq!(
        generator.inputs.load(Ordering::Relaxed),
        count * 2 + baseline_rechecks
    );
    assert_eq!(replayed, matches);
    println!(
        "{}",
        json!({"name":name,"query":document,"policy":"production",
            "start_index":start,"count":count,"batch_size":32,
            "ranking":policy.preferred().iter().map(|id|item(*id).stable_id).collect::<Vec<_>>(),
            "setup_seconds":setup_seconds,"seconds":seconds,"cpu_ticks":cpu_ticks,
            "ticks_per_second":ticks_per_second,"matches":matches,"cells":total_cells,
            "selected_cells":selected_cells,"examples":examples,"blocks":blocks,
            "validation":{"generated_inputs":generator.inputs.load(Ordering::Relaxed),
                "baseline_rechecks":baseline_rechecks,"removed_trinkets":removed_trinkets,
                "replayed_matches":replayed,"replay_depth":24,"checked_offer_decks":count,
                "forbidden_choices":0}}
        )
    );
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(32768);
    let start = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(300_000);
    let filter = args.get(3).map_or("", String::as_str);
    for (name, document) in cases()
        .into_iter()
        .filter(|(name, _)| name.contains(filter))
    {
        bench(name, &document, count, start);
    }
}
