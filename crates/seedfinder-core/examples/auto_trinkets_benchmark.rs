//! Reproducible single-thread benchmark of the shared core search engine.
//! Run `cargo run -p shpd-seedfinder-core --features json-query --release --example auto_trinkets_benchmark -- fixtures`
//! or `... -- bench 120 [name_filter]` (minimum seconds per query, both modes combined).
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::search_batch, feasibility::QueryPlan, main_world::CanonicalMainWorldGenerator,
};
use shpd_seedfinder_core::{
    catalog::{ItemId, ItemKind, item},
    challenges::Challenges,
    json_query,
    main_world::generate_main_world_with_trinket,
    seed::DungeonSeed,
    trinkets::{INITIAL_OFFER_COUNT, trinket_order},
};
use std::{collections::BTreeMap, time::Instant};

fn fixtures() {
    let mut exotic_offers = 0;
    let mut exotic_world_differences = 0;
    let mut pending = vec![
        ItemId::MossyClump,
        ItemId::TrapMechanism,
        ItemId::ExoticCrystals,
    ];
    for value in 0..1000 {
        let seed = DungeonSeed::new(value).unwrap();
        let offers = &trinket_order(seed)[..INITIAL_OFFER_COUNT];
        if !pending.iter().any(|id| offers.contains(id)) {
            continue;
        }
        let base = generate_main_world_with_trinket(seed, 24, Challenges::NONE, None).unwrap();
        pending.retain(|&id| {
            if !offers.contains(&id) { return true; }
            let branch = generate_main_world_with_trinket(seed, 24, Challenges::NONE, Some(id)).unwrap();
            if id == ItemId::ExoticCrystals {
                exotic_offers += 1;
                exotic_world_differences += usize::from(branch != base);
            }
            for found in &branch.items {
                if item(found.item).kind == ItemKind::Trinket { continue; }
                let mut req = json!({"item": item(found.item).stable_id, "uncursed": !found.cursed});
                if found.displayed_upgrade() > 0 { req["upgrade"] = json!({"exact":found.displayed_upgrade()}); }
                if let Some(effect) = found.effect { req["effect"] = json!(effect.wire_name()); }
                let query = json!({"max_depth": found.depth, "requirements": [req]});
                let Ok(parsed) = json_query::decode(&query.to_string()) else { continue; };
                if parsed.matches(&branch) && !parsed.matches(&base) {
                    println!("{}", json!({"trinket":item(id).stable_id,"seed":value,"code":seed.to_code(),"query":query,"item":format!("{found:?}"),"offers":offers.iter().map(|id|item(*id).stable_id).collect::<Vec<_>>() }));
                    return false;
                }
            }
            true
        });
        if pending.is_empty() {
            return;
        }
    }
    println!("Unproven: {pending:?}");
    println!(
        "Exotic Crystals: {exotic_offers} initial offers scanned; {exotic_world_differences} searchable world differences"
    );
}

fn timed_range(query: &Value, auto: bool, start: u64, count: u64) -> (f64, Vec<Value>) {
    let query = json_query::decode(&query.to_string()).unwrap();
    let plan = QueryPlan::analyze(&query);
    let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
    let seeds: Vec<_> = (start..start + count)
        .map(|value| DungeonSeed::new(value).unwrap())
        .collect();
    let clock = Instant::now();
    let found = search_batch(&generator, &query, &plan, &seeds, auto);
    let elapsed = clock.elapsed().as_secs_f64();
    let matches = found.into_iter().flatten().map(|m|json!({"value":m.world.seed.value(),"code":m.world.seed.to_code(),"selectedTrinket":m.selected_trinket.map(|id|item(id).stable_id)})).collect();
    (elapsed, matches)
}

fn bench(seconds: f64, name_filter: Option<&str>) {
    let queries = [
        (
            "ring_might_plus2_depth9",
            json!({"max_depth":9,"requirements":[{"item":"ring_might","upgrade":{"exact":2}}]}),
        ),
        (
            "grim_runic_blade_plus3_depth19",
            json!({"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":{"exact":3},"effect":"Grim"}]}),
        ),
        (
            "grim_runic_blade_plus3_depth24",
            json!({"max_depth":24,"requirements":[{"item":"runic_blade","upgrade":{"exact":3},"effect":"Grim"}]}),
        ),
        (
            "plate_armor_plus3_depth19",
            json!({"max_depth":19,"requirements":[{"item":"plate_armor","upgrade":{"exact":3}}]}),
        ),
        (
            "ethereal_chains_depth9",
            json!({"max_depth":9,"requirements":[{"item":"ethereal_chains"}]}),
        ),
        (
            "grim_weapon_plus1_depth19",
            json!({"max_depth":19,"requirements":[{"kind":"weapon","upgrade":{"exact":1},"effect":"Grim"}]}),
        ),
    ];
    for (name, query) in queries {
        if name_filter.is_some_and(|filter| !name.contains(filter)) {
            continue;
        }
        // Warm both execution paths before timing. Alternate the measured order per chunk.
        timed_range(&query, false, 999_000, 8);
        timed_range(&query, true, 999_000, 8);
        let mut times = [0.0; 2];
        let mut results = [Vec::new(), Vec::new()];
        let mut count = 0;
        while times.iter().sum::<f64>() < seconds {
            for mode in if count % 64 == 0 { [0, 1] } else { [1, 0] } {
                let (elapsed, found) = timed_range(&query, mode == 1, count, 32);
                times[mode] += elapsed;
                results[mode].extend(found);
            }
            count += 32;
        }
        let base: std::collections::BTreeSet<_> = results[0]
            .iter()
            .map(|v| v["value"].as_u64().unwrap())
            .collect();
        let auto: std::collections::BTreeSet<_> = results[1]
            .iter()
            .map(|v| v["value"].as_u64().unwrap())
            .collect();
        let mut branches = BTreeMap::new();
        for r in &results[1] {
            if let Some(id) = r["selectedTrinket"].as_str() {
                *branches.entry(id.to_owned()).or_insert(0) += 1;
            }
        }
        println!(
            "{}",
            json!({"name":name,"query":query,"start":0,"end":count,"baseline_seconds":times[0],"auto_seconds":times[1],"baseline_matches":base.len(),"auto_matches":auto.len(),"auto_only":auto.difference(&base).count(),"baseline_only":base.difference(&auto).count(),"branches":branches,"first_baseline":results[0].first(),"first_auto":results[1].first()})
        );
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("fixtures") => fixtures(),
        _ => bench(
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120.0),
            args.get(3).map(String::as_str),
        ),
    }
}
