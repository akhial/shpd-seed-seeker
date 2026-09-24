//! Opt-in startup benchmark; run with --release and a share link or JSON query.
//! `--queries FILE` evaluates an array of {"name": ..., "query": ...} cases.
//! Start a new process per timing sample to include cold process caches.
use serde_json::{Value, json};
use shpd_seedfinder_core::{
    auto_trinkets::AutoTrinketPolicy, catalog::item, deep_link, feasibility::QueryPlan, json_query,
    probability::estimate_match_probability, query::SearchQuery,
};
use std::time::Instant;

fn measure(name: &str, query: &SearchQuery) -> Value {
    let started = Instant::now();
    let plan = QueryPlan::analyze(query);
    let plan_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let probability = estimate_match_probability(query);
    let probability_ms = started.elapsed().as_secs_f64() * 1000.0;
    let preferred = AutoTrinketPolicy::prepare(query).map(|policy| {
        policy
            .preferred()
            .iter()
            .map(|&id| item(id).stable_id)
            .collect::<Vec<_>>()
    });
    let started = Instant::now();
    let _ = QueryPlan::analyze(query);
    let _ = estimate_match_probability(query);
    let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
    json!({
        "name": name,
        "query": json_query::encode(query),
        "plan_ms": plan_ms,
        "probability_ms": probability_ms,
        "warm_ms": warm_ms,
        "probability": probability,
        "probability_bits": format!("{:016x}", probability.to_bits()),
        "preferred": preferred,
        "impossible": plan.is_unsatisfiable(),
    })
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("run with --release");
        std::process::exit(1);
    }
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("share link, JSON query, or --queries FILE");
    let cases = if input == "--queries" {
        let data = std::fs::read_to_string(args.next().expect("query file")).unwrap();
        serde_json::from_str::<Vec<Value>>(&data)
            .unwrap()
            .into_iter()
            .map(|case| {
                (
                    case["name"].as_str().unwrap().to_owned(),
                    json_query::decode(&case["query"].to_string()).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    } else {
        let query = if input.trim_start().starts_with('{') {
            json_query::decode(&input)
        } else {
            deep_link::decode_text(&input)
        }
        .unwrap();
        vec![("query".to_owned(), query)]
    };
    for (name, query) in cases {
        // Do not let thread-local intermediate results hide another case's work.
        let result = std::thread::spawn(move || measure(&name, &query))
            .join()
            .unwrap();
        println!("{result}");
    }
}
