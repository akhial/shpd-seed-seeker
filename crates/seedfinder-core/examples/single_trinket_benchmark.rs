//! Research harness only: select one offered trinket before generating a seed.
//! `cargo run --release -p shpd-seedfinder-core --features json-query --example single_trinket_benchmark -- rank`
//! `... -- bench COUNT START [QUERY_FILTER] [POLICY]`
//! Preferred policies: ranked (default), parchment, or one stable trinket ID.
//! COUNT is fixed in advance; both modes scan the same dispersed seeds.
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    catalog::{Effect, ItemId, ItemKind, item},
    feasibility::QueryPlan,
    json_query,
    main_world::{CanonicalMainWorldGenerator, generate_main_world_with_trinket},
    model::WorldItem,
    probability::estimate_match_probability,
    query::{EffectRequirement, SearchQuery},
    quests::QuestSummary,
    search::{FloorGate, PRODUCTION_SEARCH_START_STRIDE, WorldGenerator},
    seed::{DungeonSeed, TOTAL_SEEDS},
    trinkets::{INITIAL_OFFER_COUNT, trinket_order},
};
use std::{collections::BTreeMap, time::Instant};

const CANDIDATES: [ItemId; 7] = [
    ItemId::ParchmentScrap,
    ItemId::MimicTooth,
    ItemId::RatSkull,
    ItemId::CrackedSpyglass,
    ItemId::MossyClump,
    ItemId::TrapMechanism,
    ItemId::ExoticCrystals,
];

fn cases() -> Vec<(&'static str, Value)> {
    vec![
        (
            "ring_might_plus2_depth9",
            json!({"max_depth":9,"requirements":[{"item":"ring_might","upgrade":2}]}),
        ),
        (
            "grim_weapon_plus1_depth19",
            json!({"max_depth":19,"requirements":[{"kind":"melee_weapon","upgrade":1,"effect":"Grim"}]}),
        ),
        (
            "grim_runic_blade_plus3_depth19",
            json!({"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":3,"effect":"Grim"}]}),
        ),
        (
            "grim_runic_blade_plus1_depth19",
            json!({"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]}),
        ),
        (
            "grim_runic_blade_plus3_depth24",
            json!({"max_depth":24,"requirements":[{"item":"runic_blade","upgrade":3,"effect":"Grim"}]}),
        ),
        (
            "plate_armor_plus3_depth19",
            json!({"max_depth":19,"requirements":[{"item":"plate_armor","upgrade":3}]}),
        ),
        (
            "ethereal_chains_depth9",
            json!({"max_depth":9,"requirements":[{"item":"ethereal_chains"}]}),
        ),
        (
            "fireblast_plus3_depth9",
            json!({"max_depth":9,"requirements":[{"item":"wand_fireblast","upgrade":3}]}),
        ),
        (
            "thorns_armor_plus1_depth9",
            json!({"max_depth":9,"requirements":[{"kind":"armor","upgrade":1,"effect":"Thorns"}]}),
        ),
        (
            "annoying_weapon_plus1_depth9",
            json!({"max_depth":9,"requirements":[{"kind":"melee_weapon","upgrade":1,"effect":"Annoying"}]}),
        ),
        (
            "grim_and_might_depth9",
            json!({"max_depth":9,"requirements":[{"kind":"melee_weapon","upgrade":1,"effect":"Grim"},{"item":"ring_might","upgrade":2}]}),
        ),
    ]
}

fn scores(document: &Value) -> Vec<(ItemId, f64)> {
    CANDIDATES
        .into_iter()
        .map(|id| {
            let mut selected = document.clone();
            selected["requirements"]
                .as_array_mut()
                .unwrap()
                .push(json!({
                    "item":item(id).stable_id,"select_trinket":true
                }));
            // The added AND requirement has exact offer probability 4/17.
            // Dividing it out obtains the existing conditional equipment profile.
            let p = estimate_match_probability(&json_query::decode(&selected.to_string()).unwrap())
                * 17.0
                / 4.0;
            (id, p)
        })
        .collect()
}

fn ranking(query: &SearchQuery, scores: &[(ItemId, f64)], policy: &str) -> Vec<ItemId> {
    if query
        .requirements
        .iter()
        .any(|r| r.kind == ItemKind::Trinket)
        || query.max_depth < 3
    {
        return vec![];
    }
    let curse_query = query.requirements.iter().any(|r| match r.effect {
        EffectRequirement::OneOf(set) => set.effects().any(Effect::is_curse),
        EffectRequirement::Any => false,
    });
    let base = estimate_match_probability(query);
    let mut eligible: Vec<_> = scores
        .iter()
        .copied()
        .filter(|&(id, p)| {
            if id == ItemId::ParchmentScrap && curse_query {
                return false;
            }
            match policy {
                // Limit ranked policy to effects with a directional loot benefit.
                // Do not promote neutral RNG changes based on calibration noise.
                "ranked" => CANDIDATES[..4].contains(&id) && p > base * 1.05,
                "parchment" => id == ItemId::ParchmentScrap && p > base * 1.05,
                single => item(id).stable_id == single,
            }
        })
        .collect();
    eligible.sort_by(|a, b| b.1.total_cmp(&a.1));
    eligible.into_iter().map(|(id, _)| id).collect()
}

struct PickGate<'a> {
    plan: &'a QueryPlan,
    ranking: &'a [ItemId],
    fallback: &'a [ItemId],
}
impl FloorGate for PickGate<'_> {
    fn selected_trinket(&self, seed: DungeonSeed) -> Option<ItemId> {
        if self.fallback.is_empty() {
            return self.plan.selected_trinket(seed);
        }
        let offers = trinket_order(seed);
        self.ranking
            .iter()
            .copied()
            .find(|id| offers[..INITIAL_OFFER_COUNT].contains(id))
            .or_else(|| {
                offers[..INITIAL_OFFER_COUNT].iter().copied().find(|id| {
                    // Exotic conversion preserves searchable equipment and RNG draws;
                    // the other ten neutral offers have no implemented world effect.
                    !CANDIDATES[..6].contains(id)
                })
            })
            .or_else(|| {
                self.fallback
                    .iter()
                    .copied()
                    .find(|id| offers[..INITIAL_OFFER_COUNT].contains(id))
            })
    }
    fn continue_after_floor(&self, depth: u8, items: &[WorldItem], quests: &QuestSummary) -> bool {
        self.plan.continue_after_floor(depth, items, quests)
    }
    fn wants_vault_treasure(&self) -> bool {
        self.plan.wants_vault_treasure()
    }
}

fn seed_at(index: u64) -> DungeonSeed {
    // Distinct dispersed seeds, disjoint from the calibration's uniform grid.
    DungeonSeed::new(
        u64::try_from(
            (u128::from(index) * u128::from(PRODUCTION_SEARCH_START_STRIDE) + 812_345_678_901)
                % u128::from(TOTAL_SEEDS),
        )
        .expect("modulo a u64 seed count fits u64"),
    )
    .unwrap()
}

fn process_ticks() -> u64 {
    // Linux-only supplementary CPU-time measurement. Wall time is portable.
    // Reading outside each timed call avoids charging /proc access to search.
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|stat| {
            let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
            Some(fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?)
        })
        .unwrap_or(0)
}

#[allow(clippy::too_many_lines)] // Keep the paired timing and accounting together.
fn bench(count: u64, start: u64, filter: &str, policy: &str) {
    let ticks_per_second = std::process::Command::new("getconf")
        .arg("CLK_TCK")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    for (name, document) in cases()
        .into_iter()
        .filter(|(name, _)| name.contains(filter))
    {
        let query = json_query::decode(&document.to_string()).unwrap();
        let setup = Instant::now();
        let estimates = scores(&document);
        let ranked = ranking(&query, &estimates, policy);
        let mut fallback = estimates.clone();
        fallback.sort_by(|a, b| b.1.total_cmp(&a.1));
        let fallback: Vec<_> = fallback
            .into_iter()
            .map(|(id, _)| id)
            .filter(|&id| {
                !query
                    .requirements
                    .iter()
                    .any(|r| r.kind == ItemKind::Trinket)
                    && (id != ItemId::ParchmentScrap
                        || !query.requirements.iter().any(|r| match r.effect {
                            EffectRequirement::OneOf(set) => set.effects().any(Effect::is_curse),
                            EffectRequirement::Any => false,
                        }))
            })
            .collect();
        let setup_seconds = setup.elapsed().as_secs_f64();
        let plan = QueryPlan::analyze(&query);
        assert!(!plan.is_unsatisfiable());
        let auto = PickGate {
            plan: &plan,
            ranking: &ranked,
            fallback: &fallback,
        };
        let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
        let gates: [&dyn FloorGate; 2] = [&plan, &auto];
        let warm: Vec<_> = (9_000_000..9_000_032).map(seed_at).collect();
        for gate in gates {
            generator.generate_batch_gated(&warm, plan.generation_depth(), gate);
        }
        let mut times = [0.0; 2];
        let mut cpu_ticks = [0u64; 2];
        let mut hits = [0u64; 2];
        let mut overlap = [0u64; 4]; // neither, baseline only, auto only, both
        let mut chosen = BTreeMap::<String, u64>::new();
        let mut chosen_matches = BTreeMap::<String, u64>::new();
        let mut chosen_cells = BTreeMap::<String, [u64; 4]>::new();
        let mut examples = Vec::new();
        let mut blocks = Vec::new();
        let mut done = 0u64;
        let mut first = [None, None];
        while done < count {
            let seeds: Vec<_> = (start + done..start + (done + 32).min(count))
                .map(seed_at)
                .collect();
            let mut flags = [vec![false; seeds.len()], vec![false; seeds.len()]];
            let mut block_seconds = [0.0; 2];
            let mut block_ticks = [0u64; 2];
            for mode in if done % 64 == 0 { [0, 1] } else { [1, 0] } {
                let before_ticks = process_ticks();
                let clock = Instant::now();
                let worlds =
                    generator.generate_batch_gated(&seeds, plan.generation_depth(), gates[mode]);
                for (i, world) in worlds.iter().enumerate() {
                    flags[mode][i] = world.as_ref().is_some_and(|world| query.matches(world));
                }
                drop(worlds); // Destruction belongs to search cost.
                block_seconds[mode] = clock.elapsed().as_secs_f64();
                block_ticks[mode] = process_ticks() - before_ticks;
                cpu_ticks[mode] += block_ticks[mode];
                times[mode] += block_seconds[mode];
                let n = flags[mode].iter().filter(|&&matched| matched).count() as u64;
                hits[mode] += n;
                if first[mode].is_none() {
                    first[mode] = flags[mode].iter().position(|&hit| hit).map(|i| {
                        json!({
                            "index":start+done+i as u64,"seed":seeds[i].to_code(),
                            "seconds_at_batch_end":times[mode]
                        })
                    });
                }
            }
            let mut cells = [0u64; 4];
            for (i, &seed) in seeds.iter().enumerate() {
                let cell = usize::from(flags[0][i]) + 2 * usize::from(flags[1][i]);
                cells[cell] += 1;
                overlap[cell] += 1;
                let selected = auto.selected_trinket(seed);
                let label = selected.map_or("none", |id| item(id).stable_id).to_owned();
                chosen_cells.entry(label.clone()).or_default()[cell] += 1;
                *chosen.entry(label.clone()).or_default() += 1;
                if flags[1][i] {
                    *chosen_matches.entry(label).or_default() += 1;
                }
                if cell == 2 && examples.len() < 3 {
                    // Independently replay the full scout, outside the timer.
                    let full = generate_main_world_with_trinket(
                        seed,
                        query.max_depth,
                        query.challenges,
                        selected,
                    )
                    .unwrap();
                    let base = generate_main_world_with_trinket(
                        seed,
                        query.max_depth,
                        query.challenges,
                        None,
                    )
                    .unwrap();
                    assert!(query.matches(&full) && !query.matches(&base));
                    examples.push(json!({"seed":seed.to_code(),"value":seed.value(),
                        "trinket":selected.map(|id| item(id).stable_id),
                        "items":full.items.iter().filter(|entry| query.requirements.iter().any(|r| r.matches(entry)))
                            .map(|entry| format!("{entry:?}")).collect::<Vec<_>>()
                    }));
                }
            }
            blocks.push(json!({"n":seeds.len(),"seconds":block_seconds,"cpu_ticks":block_ticks,"cells":cells}));
            done += seeds.len() as u64;
            if done % 8192 == 0 {
                eprintln!("{name} {policy}: {done}/{count}, matches {hits:?}, seconds {times:?}");
            }
        }
        println!(
            "{}",
            json!({"name":name,"query":document,"policy":policy,"start_index":start,"count":count,
                "ranking":ranked.iter().map(|id| item(*id).stable_id).collect::<Vec<_>>(),
                "setup_seconds":setup_seconds,"seconds":times,"matches":hits,"cells":overlap,
                "cpu_ticks":cpu_ticks,"ticks_per_second":ticks_per_second,
            "selected":chosen,"selected_matches":chosen_matches,"selected_cells":chosen_cells,
            "first":first,"examples":examples,"blocks":blocks
            })
        );
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "rank") {
        for (name, doc) in cases() {
            let query = json_query::decode(&doc.to_string()).unwrap();
            let estimates = scores(&doc);
            println!(
                "{}",
                json!({"name":name,"baseline":estimate_match_probability(&query),
                    "profiles":estimates.iter().map(|(id,p)|(item(*id).stable_id,*p)).collect::<BTreeMap<_,_>>(),
                    "ranking":ranking(&query,&estimates,"ranked").iter().map(|id|item(*id).stable_id).collect::<Vec<_>>()
                })
            );
        }
    } else {
        bench(
            args.get(2).and_then(|s| s.parse().ok()).unwrap_or(32768),
            args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0),
            args.get(4).map_or("", String::as_str),
            args.get(5).map_or("ranked", String::as_str),
        );
    }
}
