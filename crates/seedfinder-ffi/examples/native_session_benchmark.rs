//! Shared native session including trinket replay, with explicit polling cadence.
//! Usage: `native_session_benchmark QUERY_JSON WORKERS CHUNK START COUNT POLL_MS DRAIN_SIZE DRAIN_MODE`
use serde_json::json;
use shpd_seedfinder_core::{
    json_query, main_world::CanonicalMainWorldGenerator, search::SearchOptions,
};
use shpd_seedfinder_session::{NativeSession, STATE_COMPLETED, STATE_RUNNING};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(clippy::too_many_lines)]
fn main() {
    assert!(shpd_seedfinder_ffi::seedfinder_available_workers() > 0);
    let args: Vec<_> = std::env::args().collect();
    let query = json_query::decode(&args[1]).expect("valid query");
    let workers = NonZeroUsize::new(args[2].parse().expect("workers")).unwrap();
    let chunk_size = NonZeroUsize::new(args[3].parse().expect("chunk")).unwrap();
    let start_seed: u64 = args[4].parse().expect("start");
    let count: u64 = args[5].parse().expect("count");
    let poll_ms: u64 = args[6].parse().expect("poll period");
    let drain_size: usize = args[7].parse().expect("drain size");
    assert!(drain_size > 0);
    let mode = args.get(8).map_or("eager", String::as_str);
    assert!(matches!(mode, "eager" | "single"));
    let generator = Arc::new(CanonicalMainWorldGenerator::with_challenges(
        query.challenges,
    ));
    let options = SearchOptions {
        start_seed,
        end_seed_exclusive: start_seed + count,
        workers,
        chunk_size,
        max_results: NonZeroUsize::MAX,
    };
    let began = Instant::now();
    let session = NativeSession::start(&generator, query, options).unwrap();
    let mut recipes = Vec::new();
    let mut polls = 0_u64;
    let mut drain_calls = 0_u64;
    let mut all_tested_observed = None;
    loop {
        polls += 1;
        loop {
            observe_all_tested(&session, began, &mut all_tested_observed);
            let matches = session.drain_matches(drain_size).unwrap();
            drain_calls += 1;
            observe_all_tested(&session, began, &mut all_tested_observed);
            let empty = matches.is_empty();
            recipes.extend(
                matches
                    .into_iter()
                    .map(|matched| (matched.recipe.seed.value(), matched.recipe.trinket)),
            );
            if empty || mode == "single" {
                break;
            }
        }
        if session.status()[0] != STATE_RUNNING {
            // Terminal status already guarantees an empty worker queue.
            // Retain a defensive drain as an assertion of that API contract.
            let pending = session.drain_matches(drain_size).unwrap();
            drain_calls += 1;
            assert!(
                pending.is_empty(),
                "terminal status must have an empty queue"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
    }
    observe_all_tested(&session, began, &mut all_tested_observed);
    let seconds = began.elapsed().as_secs_f64();
    let status = session.status();
    assert_eq!(
        status[0],
        STATE_COMPLETED,
        "{:?}",
        session.take_failure_diagnostic()
    );
    assert_eq!(status[1], i64::try_from(count).unwrap());
    assert_eq!(status[2], i64::try_from(count).unwrap());
    assert_eq!(
        session.resume_hint(),
        [i64::try_from(start_seed).unwrap(), 0]
    );
    if workers.get() == 1 {
        assert!(recipes.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }
    recipes.sort_unstable_by_key(|recipe| recipe.0);
    assert!(
        recipes
            .iter()
            .all(|recipe| (start_seed..start_seed + count).contains(&recipe.0))
    );
    assert!(recipes.windows(2).all(|pair| pair[0].0 < pair[1].0));
    let matches: Vec<_> = recipes
        .into_iter()
        .map(
            |(seed, trinket)| json!({"seed":seed,"trinket":trinket.map(|item|format!("{item:?}"))}),
        )
        .collect();
    println!(
        "{}",
        json!({"seconds":seconds,"tested":count,"matches":matches,
        "workers":workers.get(),"chunk_size":chunk_size.get(),"start":start_seed,
        "poll_ms":poll_ms,"drain_size":drain_size,"drain_mode":mode,"polls":polls,"drain_calls":drain_calls,
        "all_tested_observed_seconds":all_tested_observed.expect("completed progress observed"),
        "drain_tail_observed_seconds":seconds-all_tested_observed.unwrap()})
    );
}

// Public progress is sampled around drains; this is not an exact worker-stop timestamp.
fn observe_all_tested(session: &NativeSession, began: Instant, observed: &mut Option<f64>) {
    if observed.is_none() {
        let status = session.status();
        if status[1] == status[2] {
            *observed = Some(began.elapsed().as_secs_f64());
        }
    }
}
