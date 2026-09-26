//! Held-out, streaming probability audit. No generated worlds are retained.
//! `cargo run --release --example probability_sweep -- 32768 none > audit.json`
//! Replace `none` with a selected trinket ID to audit that profile, including
//! the chance of its initial offer. `--recheck audit.json` re-estimates saved
//! observations without regenerating worlds. Progress goes to stderr.
//! Use `auto` to replay the actual query-aware `AutoTrinket` choice per seed.
//! An optional fourth positional argument supplies a JSON array of queries
//! for a focused diagnostic, generated only through their deepest limit.
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::{AutoTrinketPolicy, CANDIDATES},
    catalog::{ItemId, item_by_stable_id},
    challenges::Challenges,
    floor_filters::{CompiledFloorRequirement, RoomType},
    json_query,
    main_world::generate_main_world_with_trinket,
    probability::estimate_match_probability,
    query::SearchQuery,
    seed::{DungeonSeed, TOTAL_SEEDS},
};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

struct Case {
    category: &'static str,
    query: SearchQuery,
    floors: Vec<(u8, CompiledFloorRequirement)>,
}

#[allow(clippy::needless_pass_by_value)] // Own the temporary JSON documents used by the corpus builder.
fn push(cases: &mut Vec<Case>, category: &'static str, document: Value) {
    let query = json_query::decode(&document.to_string()).expect("valid sweep query");
    let floors = query
        .floor_requirements
        .iter()
        .map(|f| (f.depth, f.compile()))
        .collect();
    cases.push(Case {
        category,
        query,
        floors,
    });
}

fn farm(depth: u8) -> Value {
    json!({"depth":depth,"feeling":"dark","any_rooms":["garden","secret_garden"]})
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for depth in (1..=24).filter(|depth| depth % 5 != 0) {
        for feeling in [
            "none", "chasm", "water", "grass", "dark", "large", "traps", "secrets",
        ] {
            // Dark on floor 1 is deliberately impossible, but is a valid query.
            push(
                &mut cases,
                "feeling",
                json!({"requirements":[],"floor_requirements":[{"depth":depth,"feeling":feeling}]}),
            );
        }
        for room in RoomType::ALL {
            push(
                &mut cases,
                "room",
                json!({"requirements":[],"floor_requirements":[{"depth":depth,"rooms":[room.stable_id()]}]}),
            );
        }
        for feeling in ["none", "dark", "large", "secrets"] {
            for rooms in [
                vec!["garden"],
                vec!["secret_garden"],
                vec!["garden", "secret_garden"],
                vec!["secret_library", "secret_laboratory"],
                vec!["garden", "library"],
                vec!["laboratory", "secret_laboratory"],
                vec!["garden", "secret_garden", "library"],
                vec![
                    "secret_library",
                    "secret_laboratory",
                    "secret_garden",
                    "secret_larder",
                ],
                vec!["standard_plants", "standard_striped", "standard_study"],
            ] {
                for mode in ["rooms", "any_rooms"] {
                    push(
                        &mut cases,
                        "room_combination",
                        json!({"requirements":[],"floor_requirements":[{"depth":depth,"feeling":feeling,mode:rooms}]}),
                    );
                }
            }
        }
        if depth < 24 && (depth + 1) % 5 != 0 {
            for room in [
                "garden",
                "secret_garden",
                "library",
                "laboratory",
                "quest_rot_garden",
            ] {
                push(
                    &mut cases,
                    "cross_floor",
                    json!({"requirements":[],"floor_requirements":[{"depth":depth,"rooms":[room]},{"depth":depth+1,"rooms":[room]}]}),
                );
            }
            for feeling in ["dark", "grass", "secrets"] {
                push(
                    &mut cases,
                    "cross_floor",
                    json!({"requirements":[],"floor_requirements":[{"depth":depth,"feeling":feeling},{"depth":depth+1,"feeling":feeling}]}),
                );
            }
        }
    }
    for mask in 1..8 {
        let floors: Vec<_> = [7, 17, 22]
            .into_iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, depth)| farm(depth))
            .collect();
        push(
            &mut cases,
            "farming",
            json!({"requirements":[],"floor_requirements":floors}),
        );
        for upgrade in [0, 1, 2] {
            push(
                &mut cases,
                "mixed_farming",
                json!({"requirements":[{"item":"ring_wealth","upgrade":if upgrade == 0 { json!("any") } else { json!(upgrade) },"max_depth":16}],"floor_requirements":floors}),
            );
        }
    }
    item_cases(&mut cases);
    first_floor_cases(&mut cases);
    cases
}

fn first_floor_cases(cases: &mut Vec<Case>) {
    for key in 0..625 {
        let counts = [1, 5, 25, 125].map(|power| key / power % 5);
        if counts.iter().sum::<usize>() > 4 || counts.iter().filter(|count| **count > 0).count() < 2
        {
            continue;
        }
        for narrow in [false, true] {
            let mut requirements = Vec::new();
            for (count, kind) in counts.into_iter().zip(["weapon", "armor", "wand", "ring"]) {
                for _ in 0..count {
                    requirements.push(if narrow {
                        json!({"kind":kind,"upgrade":{"at_least":1}})
                    } else {
                        json!({"kind":kind})
                    });
                }
            }
            push(
                cases,
                "first_floor",
                json!({"max_depth":1,"requirements":requirements}),
            );
        }
    }
}

#[allow(clippy::too_many_lines)] // Explicit sweep matrix, not application control flow.
fn item_cases(cases: &mut Vec<Case>) {
    for depth in [4, 9, 14, 19, 24] {
        for kind in ["weapon", "thrown_weapon", "armor", "wand", "ring"] {
            for upgrade in [0, 1, 2, 3] {
                push(
                    cases,
                    "equipment",
                    json!({"max_depth":depth,"requirements":[{"kind":kind,"upgrade":if upgrade == 0 { json!("any") } else { json!(upgrade) }}]}),
                );
                for source in [
                    "heap",
                    "chest",
                    "shop",
                    "ghost_reward",
                    "wandmaker_reward",
                    "blacksmith_reward",
                    "imp_reward",
                    "vault_treasure",
                ] {
                    let document = json!({"max_depth":depth,"requirements":[{"kind":kind,"upgrade":if upgrade == 0 { json!("any") } else { json!(upgrade) },"source":source}]});
                    if json_query::decode(&document.to_string()).is_ok() {
                        push(cases, "source", document);
                    }
                }
            }
        }
        for item in [
            "ring_wealth",
            "ring_might",
            "ring_haste",
            "ring_force",
            "wand_corruption",
            "wand_regrowth",
            "wand_fireblast",
            "plate_armor",
            "runic_blade",
            "ethereal_chains",
            "sandals_of_nature",
        ] {
            push(
                cases,
                "identity",
                json!({"max_depth":depth,"requirements":[{"item":item}]}),
            );
        }
        for items in [
            vec!["ring_wealth", "ring_might"],
            vec!["ethereal_chains", "sandals_of_nature"],
            vec!["wand_fireblast", "ring_wealth"],
            vec!["ring_wealth", "ring_wealth"],
        ] {
            let requirements: Vec<_> = items.iter().map(|item| json!({"item":item})).collect();
            push(
                cases,
                "competition",
                json!({"max_depth":depth,"requirements":requirements}),
            );
        }
    }
    for definition in shpd_seedfinder_core::catalog::ITEMS.iter().filter(|item| {
        matches!(
            item.kind,
            shpd_seedfinder_core::catalog::ItemKind::Ring
                | shpd_seedfinder_core::catalog::ItemKind::Wand
        )
    }) {
        for upgrade in [0, 1, 2, 3] {
            push(
                cases,
                "vault_identity",
                json!({"requirements":[{"item":definition.stable_id,"source":"vault_treasure","upgrade":if upgrade == 0 { json!("any") } else { json!(upgrade) }}]}),
            );
        }
    }
    for floor in [7, 17, 22] {
        for item in [
            "ring_wealth",
            "ring_might",
            "wand_fireblast",
            "ethereal_chains",
        ] {
            for upgrade in [0, 1, 2] {
                if item == "ethereal_chains" && upgrade > 0 {
                    continue;
                }
                push(
                    cases,
                    "mixed",
                    json!({"requirements":[{"item":item,"upgrade":if upgrade == 0 { json!("any") } else { json!(upgrade) },"max_depth":floor-1}],"floor_requirements":[farm(floor)]}),
                );
            }
        }
    }
}

#[allow(clippy::too_many_lines)] // Streaming audit setup, shared generation, and report serialization.
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--recheck") {
        recheck(&args[1]);
        return;
    }
    let samples: usize = args.first().map_or(32768, |s| s.parse().unwrap());
    assert!(samples > 0);
    let profile = args.get(1).map_or("none", String::as_str);
    let selected: Option<ItemId> = (!matches!(profile, "none" | "auto"))
        .then(|| item_by_stable_id(profile).expect("trinket ID").id);
    let seed_offset = args
        .get(2)
        .map_or(918_273_645, |s| s.parse::<u64>().unwrap())
        % TOTAL_SEEDS;
    let mut cases = load_cases(args.get(3).map(String::as_str));
    for case in &mut cases {
        case.query.auto_apply_trinket = profile == "auto";
    }
    if let Some(id) = selected {
        for case in &mut cases {
            let mut document = json_query::encode(&case.query);
            document["requirements"].as_array_mut().unwrap().push(json!({"item":shpd_seedfinder_core::catalog::item(id).stable_id,"select_trinket":true}));
            case.query = json_query::decode(&document.to_string()).unwrap();
        }
    }
    let started = Instant::now();
    // Measure before preparing policies, which intentionally reuse these scores.
    let estimates: Vec<_> = cases
        .iter()
        .map(|case| {
            let time = Instant::now();
            let estimate = estimate_match_probability(&case.query);
            (estimate, time.elapsed().as_nanos())
        })
        .collect();
    let policies: Vec<_> = cases
        .iter()
        .map(|case| AutoTrinketPolicy::prepare(&case.query))
        .collect();
    let generation_depth = cases.iter().map(|case| case.query.max_depth).max().unwrap();
    let cursor = AtomicUsize::new(0);
    let counts = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..std::thread::available_parallelism().unwrap().get())
            .map(|_| {
                scope.spawn(|| {
                    let mut hits = vec![0u32; cases.len()];
                    loop {
                        let index = cursor.fetch_add(1, Ordering::Relaxed);
                        if index >= samples {
                            break;
                        }
                        // Offset from both the item training grid and the floor corpus.
                        let seed = audit_seed(seed_offset, index);
                        if selected.is_some_and(|id| {
                            !shpd_seedfinder_core::trinkets::trinket_order(seed)[..4].contains(&id)
                        }) {
                            continue;
                        }
                        let choices: Vec<_> = policies
                            .iter()
                            .map(|policy| {
                                policy
                                    .as_ref()
                                    .map_or(selected, |policy| policy.selected_trinket(seed))
                            })
                            .collect();
                        for choice in std::iter::once(selected).chain(CANDIDATES.map(Some)) {
                            if !choices.contains(&choice) {
                                continue;
                            }
                            // All queries choosing this profile share a single world.
                            let world = generate_main_world_with_trinket(
                                seed,
                                generation_depth,
                                Challenges::NONE,
                                choice,
                            )
                            .unwrap();
                            for ((case, count), wanted) in cases.iter().zip(&mut hits).zip(&choices)
                            {
                                if *wanted != choice {
                                    continue;
                                }
                                if !case.floors.iter().all(|(depth, filter)| {
                                    let index = usize::from(*depth - 1 - (*depth / 5));
                                    filter.matches_feeling(world.feelings[index].feeling)
                                        && filter.matches_rooms(world.floor_rooms[index].rooms)
                                }) {
                                    continue;
                                }
                                *count += u32::from(
                                    case.query.requirements.is_empty()
                                        || case.query.matches(&world),
                                );
                            }
                            if profile != "auto" {
                                break;
                            }
                        }
                        if index > 0 && index % 8192 == 0 {
                            eprintln!(
                                "{profile}: {index}/{samples} worlds in {:?}",
                                started.elapsed()
                            );
                        }
                    }
                    hits
                })
            })
            .collect();
        let mut totals = vec![0u32; cases.len()];
        for worker in workers {
            for (total, count) in totals.iter_mut().zip(worker.join().unwrap()) {
                *total += count;
            }
        }
        totals
    });
    let mut rows = Vec::new();
    let mut slowest = 0u128;
    for ((case, hits), (estimate, elapsed)) in cases.into_iter().zip(counts).zip(estimates) {
        slowest = slowest.max(elapsed);
        rows.push(json!({"category":case.category,"query":json_query::encode(&case.query),"hits":hits,"estimate":estimate,"estimate_ns":elapsed}));
    }
    eprintln!(
        "{} queries, {samples} worlds in {:?}; slowest estimate {slowest}ns",
        rows.len(),
        started.elapsed()
    );
    println!("{}", serde_json::to_string_pretty(&json!({"samples":samples,"profile":profile,"seed_offset":seed_offset,"seed_stride":3_355_211_884_971u64,"elapsed_seconds":started.elapsed().as_secs_f64(),"generation_depth":generation_depth,"cases":rows})).unwrap());
}

fn audit_seed(offset: u64, index: usize) -> DungeonSeed {
    let value = (u128::from(offset) + index as u128 * 3_355_211_884_971) % u128::from(TOTAL_SEEDS);
    DungeonSeed::new(u64::try_from(value).unwrap()).unwrap()
}

fn recheck(path: &str) {
    let mut report: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for row in report["cases"].as_array_mut().unwrap() {
        let query = json_query::decode(&row["query"].to_string()).unwrap();
        let started = Instant::now();
        row["estimate"] = json!(estimate_match_probability(&query));
        row["estimate_ns"] = json!(started.elapsed().as_nanos());
    }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn load_cases(path: Option<&str>) -> Vec<Case> {
    if let Some(path) = path {
        let documents: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut cases = Vec::new();
        for document in documents {
            push(&mut cases, "probe", document);
        }
        assert!(!cases.is_empty());
        cases
    } else {
        cases()
    }
}
