//! Measures the shared native streaming scheduler over a fixed seed interval.
//! Usage: `stream_benchmark QUERY_JSON WORKERS CHUNK START COUNT POLL_MS DRAIN_SIZE DRAIN_MODE`
use serde_json::json;
use shpd_seedfinder_core::{
    json_query,
    main_world::CanonicalMainWorldGenerator,
    search::{ResumeCoverage, SearchOptions, StreamingSearchState, spawn_streaming_search},
};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(clippy::too_many_lines)]
fn main() {
    // Link the same allocator and engine package as the native FFI consumers.
    assert!(shpd_seedfinder_ffi::seedfinder_available_workers() > 0);
    let args: Vec<_> = std::env::args().collect();
    let query = json_query::decode(&args[1]).expect("valid query");
    assert!(
        !query.auto_apply_trinket,
        "this scheduler-only adapter requires automatic trinkets off"
    );
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
    let handle = spawn_streaming_search(&generator, query, options).unwrap();
    let mut seeds = Vec::new();
    let mut polls = 0_u64;
    let mut drain_calls = 0_u64;
    let mut all_tested_observed = None;
    let observe = |observed: &mut Option<f64>| {
        if observed.is_none() && handle.tested() == count {
            *observed = Some(began.elapsed().as_secs_f64());
        }
    };
    loop {
        polls += 1;
        loop {
            observe(&mut all_tested_observed);
            let worlds = handle.drain_results(drain_size);
            drain_calls += 1;
            observe(&mut all_tested_observed);
            let empty = worlds.is_empty();
            seeds.extend(worlds.into_iter().map(|world| world.seed.value()));
            if empty || mode == "single" {
                break;
            }
        }
        if handle.state() != StreamingSearchState::Running {
            let pending = handle.drain_results(usize::MAX);
            drain_calls += 1;
            assert!(
                pending.is_empty(),
                "terminal status must have an empty queue"
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(poll_ms));
    }
    observe(&mut all_tested_observed);
    let seconds = began.elapsed().as_secs_f64();
    assert_eq!(
        handle.state(),
        StreamingSearchState::Completed,
        "{:?}",
        handle.failure()
    );
    assert_eq!(handle.tested(), count);
    assert_eq!(handle.total(), count);
    assert_eq!(handle.accepted(), u64::try_from(seeds.len()).unwrap());
    assert_eq!(
        handle.resume_coverage(),
        ResumeCoverage {
            position: start_seed,
            remaining: 0
        }
    );
    if workers.get() == 1 {
        assert!(seeds.windows(2).all(|pair| pair[0] < pair[1]));
    }
    seeds.sort_unstable();
    assert!(
        seeds
            .iter()
            .all(|&seed| (start_seed..start_seed + count).contains(&seed))
    );
    assert!(seeds.windows(2).all(|pair| pair[0] < pair[1]));
    println!(
        "{}",
        json!({"seconds":seconds,"tested":handle.tested(),"matches":seeds,
        "workers":workers.get(),"chunk_size":chunk_size.get(),"start":start_seed,
        "poll_ms":poll_ms,"drain_size":drain_size,"drain_mode":mode,"polls":polls,"drain_calls":drain_calls,
        "all_tested_observed_seconds":all_tested_observed.expect("completed progress observed"),
        "drain_tail_observed_seconds":seconds-all_tested_observed.unwrap()})
    );
}
