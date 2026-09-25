#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod json_output;

use std::collections::hash_map::RandomState;
use std::env;
use std::fs;
use std::hash::BuildHasher as _;
use std::io::{self, Write as _};
use std::num::NonZeroUsize;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use shpd_seedfinder_core::SHPD_VERSION;
use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::query::{
    EffectRequirement, Requirement, SearchQuery, TierRequirement, UpgradeRequirement,
};
use shpd_seedfinder_core::search::{SearchOptions, SearchProgress, search_parallel};
use shpd_seedfinder_core::seed::{DungeonSeed, TOTAL_SEEDS};

const DEFAULT_BENCHMARK_SEEDS: u64 = 10_000;
const SEARCH_CHUNK_SIZE: usize = 4;
const SEARCH_WINDOW_SEEDS: u64 = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
struct BenchmarkOptions {
    seeds: u64,
    workers: Option<NonZeroUsize>,
    items: Option<PathBuf>,
    random_start: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Benchmark(BenchmarkOptions),
    Search {
        items: PathBuf,
        workers: Option<NonZeroUsize>,
        output: Option<PathBuf>,
        json: bool,
        random_start: bool,
    },
    Help,
    Version,
}

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Benchmark(options)) => match benchmark_command(&options) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("seed-seeker: benchmark failed: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::Search {
            items,
            workers,
            output,
            json,
            random_start,
        }) => match search_command(&items, workers, output.as_deref(), json, random_start) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("seed-seeker: search failed: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::Help) => {
            print!("{}", help());
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!(
                "seed-seeker {} (Shattered Pixel Dungeon {SHPD_VERSION})",
                env!("CARGO_PKG_VERSION")
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("seed-seeker: {error}\n\n{}", help());
            ExitCode::from(2)
        }
    }
}

fn parse_args(arguments: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    if arguments.len() <= 1 {
        match arguments.first().map(String::as_str) {
            None | Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            _ => {}
        }
    }

    let mut benchmark_seeds = None;
    let mut workers = None;
    let mut items = None;
    let mut output = None;
    let mut json = false;
    let mut random_start = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--benchmark" | "-b" => {
                if benchmark_seeds.is_some() {
                    return Err("benchmark may only be specified once".to_owned());
                }
                let mut seeds = DEFAULT_BENCHMARK_SEEDS;
                if arguments
                    .get(index + 1)
                    .is_some_and(|argument| !argument.starts_with('-'))
                {
                    index += 1;
                    seeds = parse_seed_count(&arguments[index])?;
                }
                benchmark_seeds = Some(seeds);
            }
            "--workers" => {
                if workers.is_some() {
                    return Err("--workers may only be specified once".to_owned());
                }
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "--workers requires a positive integer".to_owned())?;
                workers = Some(parse_worker_count(value)?);
            }
            "--items" | "-i" => {
                if items.is_some() {
                    return Err("--items may only be specified once".to_owned());
                }
                index += 1;
                let value = arguments
                    .get(index)
                    .ok_or_else(|| "--items requires a JSON file path".to_owned())?;
                items = Some(PathBuf::from(value));
            }
            "--output" | "-o" => {
                if output.is_some() {
                    return Err("--output may only be specified once".to_owned());
                }
                index += 1;
                let value = arguments
                    .get(index)
                    .filter(|value| !value.is_empty() && !value.starts_with('-'))
                    .ok_or_else(|| {
                        "--output requires a file path (stdout is not supported)".to_owned()
                    })?;
                output = Some(PathBuf::from(value));
            }
            "--json" => parse_flag(&mut json, "--json")?,
            "--random-start" => parse_flag(&mut random_start, "--random-start")?,
            "--help" | "-h" | "--version" | "-V" => {
                return Err("help and version cannot be combined with other options".to_owned());
            }
            argument => return Err(format!("unknown option '{argument}'")),
        }
        index += 1;
    }

    if json && output.is_none() {
        return Err("--json requires --output FILE".to_owned());
    }
    if output.is_some() && (benchmark_seeds.is_some() || items.is_none()) {
        return Err("--output and --json require an --items search without --benchmark".to_owned());
    }
    if let Some(seeds) = benchmark_seeds {
        return Ok(Command::Benchmark(BenchmarkOptions {
            seeds,
            workers,
            items,
            random_start,
        }));
    }
    if let Some(items) = items {
        return Ok(Command::Search {
            items,
            workers,
            output,
            json,
            random_start,
        });
    }
    Err("--workers and --random-start require --benchmark or --items".to_owned())
}

fn parse_flag(flag: &mut bool, name: &str) -> Result<(), String> {
    if *flag {
        return Err(format!("{name} may only be specified once"));
    }
    *flag = true;
    Ok(())
}

fn parse_seed_count(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|seeds| (1..=TOTAL_SEEDS).contains(seeds))
        .ok_or_else(|| format!("benchmark seed count must be between 1 and {TOTAL_SEEDS}"))
}

fn parse_worker_count(value: &str) -> Result<NonZeroUsize, String> {
    value
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| "--workers requires a positive integer".to_owned())
}

fn help() -> &'static str {
    concat!(
        "Seed Seeker command-line tools\n\n",
        "Usage:\n",
        "  seed-seeker --items FILE [--random-start] [--workers WORKERS] [--output FILE [--json]]\n",
        "  seed-seeker [--items FILE] --benchmark [SEEDS] [--random-start] [--workers WORKERS]\n\n",
        "Options:\n",
        "  -b, --benchmark [SEEDS]  Benchmark a seed search\n",
        "                            [default: 10000]\n",
        "  -i, --items FILE          Read search requirements from a JSON file\n",
        "      --workers WORKERS     Number of search workers [default: available CPUs]\n",
        "      --random-start        Start at a random seed [default: AAA-AAA-AAA]\n",
        "                            Advance by 1, wrapping after ZZZ-ZZZ-ZZZ\n",
        "  -o, --output FILE         Write matching seeds to a file (replaces existing)\n",
        "      --json                Export app-importable JSON; requires --output FILE\n",
        "                            Keeps the file valid; stops at 1024 matches\n",
        "  -h, --help                Print help\n",
        "  -V, --version             Print version\n",
    )
}

fn benchmark_command(benchmark: &BenchmarkOptions) -> Result<String, String> {
    let query = benchmark
        .items
        .as_deref()
        .map(load_query)
        .transpose()?
        .unwrap_or_else(benchmark_query);
    let workers = benchmark
        .workers
        .unwrap_or_else(SearchOptions::available_parallelism);
    let start_seed = search_start(benchmark.random_start);
    let generator = CanonicalMainWorldGenerator::with_challenges(query.challenges);
    let mut tested = 0;
    let mut matches = 0;
    let mut elapsed = std::time::Duration::ZERO;
    for range in search_ranges(start_seed, benchmark.seeds, TOTAL_SEEDS) {
        let outcome = search_parallel(
            &generator,
            &query,
            SearchOptions {
                start_seed: range.start,
                end_seed_exclusive: range.end,
                workers,
                chunk_size: NonZeroUsize::new(SEARCH_CHUNK_SIZE).expect("chunk size is non-zero"),
                max_results: NonZeroUsize::MAX,
            },
            &SearchProgress::default(),
        )
        .map_err(|error| format!("{error:?}"))?;
        tested += outcome.tested;
        matches += outcome.worlds.len();
        elapsed += outcome.elapsed;
    }
    #[allow(clippy::cast_precision_loss)] // Display-only throughput.
    let throughput = if elapsed.is_zero() {
        0.0
    } else {
        tested as f64 / elapsed.as_secs_f64()
    };

    Ok(format!(
        concat!(
            "Seed Seeker benchmark (Shattered Pixel Dungeon {shpd_version})\n",
            "Workers: {workers}\n",
            "Seeds tested: {tested}\n",
            "Matches: {matches}\n",
            "Elapsed: {elapsed:.3} s\n",
            "Throughput: {throughput:.0} seeds/s",
        ),
        shpd_version = SHPD_VERSION,
        workers = workers,
        tested = tested,
        matches = matches,
        elapsed = elapsed.as_secs_f64(),
        throughput = throughput,
    ))
}

fn search_command(
    items: &Path,
    workers: Option<NonZeroUsize>,
    output: Option<&Path>,
    json: bool,
    random_start: bool,
) -> Result<(), String> {
    let query = load_query(items)?;
    if let Some(path) = output {
        if fs::canonicalize(path).ok() == fs::canonicalize(items).ok() {
            return Err("--output must be different from the --items file".to_owned());
        }
    }
    let workers = workers.unwrap_or_else(SearchOptions::available_parallelism);
    let start_seed = search_start(random_start);
    if json {
        return json_output::search(
            &Arc::new(CanonicalMainWorldGenerator::with_challenges(
                query.challenges,
            )),
            &query,
            workers,
            output.ok_or("--json requires --output FILE")?,
            start_seed,
        );
    }
    let stdout = io::stdout();
    let mut output: Box<dyn io::Write> = match output {
        Some(path) => Box::new(io::BufWriter::new(
            fs::File::create(path)
                .map_err(|error| format!("could not create '{}': {error}", path.display()))?,
        )),
        None => Box::new(io::BufWriter::new(stdout.lock())),
    };
    if QueryPlan::analyze(&query).is_unsatisfiable() {
        eprintln!(
            "seed-seeker: no seed can satisfy this query within depth {}; nothing to search",
            query.max_depth
        );
        return Ok(());
    }
    for range in search_ranges(start_seed, TOTAL_SEEDS, SEARCH_WINDOW_SEEDS) {
        let outcome = search_parallel(
            &CanonicalMainWorldGenerator::with_challenges(query.challenges),
            &query,
            SearchOptions {
                start_seed: range.start,
                end_seed_exclusive: range.end,
                workers,
                chunk_size: NonZeroUsize::new(SEARCH_CHUNK_SIZE).expect("chunk size is non-zero"),
                max_results: NonZeroUsize::MAX,
            },
            &SearchProgress::default(),
        )
        .map_err(|error| format!("{error:?}"))?;
        for world in outcome.worlds {
            writeln!(output, "{}", world.seed)
                .map_err(|error| format!("could not write matching seed: {error}"))?;
        }
        output
            .flush()
            .map_err(|error| format!("could not flush matching seeds: {error}"))?;
    }
    Ok(())
}

fn search_start(random_start: bool) -> DungeonSeed {
    if !random_start {
        return DungeonSeed::MIN;
    }
    // Match the native apps' initial traversal start. Each CLI invocation is
    // a fresh search, so it does not need the apps' between-search stride.
    let value = RandomState::new().hash_one(0_u8) % TOTAL_SEEDS;
    let seed = DungeonSeed::new(value).expect("random start is inside the seed space");
    eprintln!("seed-seeker: starting at {seed} (offset 1)");
    seed
}

/// Splits a consecutive traversal into bounded numeric ranges, wrapping once
/// at the end of the seed space. `count` is the number of seeds to visit, not
/// an absolute end seed, so a full search also covers the seeds before its start.
fn search_ranges(
    start_seed: DungeonSeed,
    mut count: u64,
    window_seeds: u64,
) -> impl Iterator<Item = Range<u64>> {
    debug_assert!(count <= TOTAL_SEEDS);
    debug_assert!(window_seeds > 0);
    let mut start = start_seed.value();
    std::iter::from_fn(move || {
        if count == 0 {
            return None;
        }
        let len = count.min(window_seeds).min(TOTAL_SEEDS - start);
        let range = start..start + len;
        count -= len;
        start = range.end % TOTAL_SEEDS;
        Some(range)
    })
}

fn load_query(path: &Path) -> Result<SearchQuery, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("could not read '{}': {error}", path.display()))?;
    shpd_seedfinder_core::json_query::decode(&contents)
        .map_err(|error| format!("could not parse '{}': {error}", path.display()))
}

/// The canonical benchmark: a +5 Runic Blade anywhere in the first 19 floors.
///
/// A tier-4 weapon among the Imp's reward options is the only thing in v4.0.0
/// that reaches +5, so the plan runs to the Imp's depth-19 deadline and no
/// quest window can end it sooner: every seed is generated through the City.
/// The vault sub-level is not built — its treasure stops at +4 — which keeps
/// the workload the plain cost of nineteen floors. Numbers published against
/// the older engine, or against the +3 Wand of Fireblast workload used before
/// it, are not comparable with current ones.
fn benchmark_query() -> SearchQuery {
    SearchQuery {
        floor_requirements: Vec::new(),
        auto_apply_trinket: false,
        arcane_resin_filter: shpd_seedfinder_core::query::ArcaneResinFilter::default(),
        arcane_resin_auto: false,
        arcane_resin: 0,
        requirements: vec![Requirement {
            kind: ItemKind::Weapon,
            weapon_category: None,
            item: Some(ItemId::RunicBlade),
            tier: TierRequirement::Any,
            upgrade: UpgradeRequirement::Exact(5),
            effect: EffectRequirement::Any,
            require_uncursed: false,
            select_trinket: false,
            trinket_transmutations: 0,
            blanket: false,
            exclude_resin: false,
            source: None,
            identity_group: None,
            max_depth: None,
            alternative_group: None,
            level_sum: None,
        }],
        max_depth: 19,
        challenges: Challenges::NONE,
        require_blacksmith: false,
        exclude_blacksmith_rewards: false,
        wandmaker_quest: None,
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::path::PathBuf;

    use shpd_seedfinder_core::seed::{DungeonSeed, TOTAL_SEEDS};

    use super::{BenchmarkOptions, Command, help, parse_args, search_ranges, search_start};

    fn benchmark(seeds: u64, workers: Option<usize>) -> Command {
        Command::Benchmark(BenchmarkOptions {
            seeds,
            workers: workers.and_then(NonZeroUsize::new),
            items: None,
            random_start: false,
        })
    }

    #[test]
    fn accepts_both_benchmark_flags_with_an_optional_seed_count() {
        assert_eq!(
            parse_args(["--benchmark".to_owned()]),
            Ok(benchmark(10_000, None))
        );
        assert_eq!(
            parse_args(["-b".to_owned(), "1000".to_owned()]),
            Ok(benchmark(1_000, None))
        );
    }

    #[test]
    fn accepts_a_worker_count_before_or_after_the_benchmark() {
        assert_eq!(
            parse_args([
                "--benchmark".to_owned(),
                "1000".to_owned(),
                "--workers".to_owned(),
                "4".to_owned(),
            ]),
            Ok(benchmark(1_000, Some(4)))
        );
        assert_eq!(
            parse_args(["--workers".to_owned(), "2".to_owned(), "-b".to_owned(),]),
            Ok(benchmark(10_000, Some(2)))
        );
    }

    #[test]
    fn defaults_to_help_and_accepts_standard_information_flags() {
        assert_eq!(parse_args([]), Ok(Command::Help));
        assert_eq!(parse_args(["--help".to_owned()]), Ok(Command::Help));
        assert_eq!(parse_args(["-h".to_owned()]), Ok(Command::Help));
        assert_eq!(parse_args(["--version".to_owned()]), Ok(Command::Version));
        assert_eq!(parse_args(["-V".to_owned()]), Ok(Command::Version));
    }

    #[test]
    fn items_file_starts_a_search_without_the_benchmark_flag() {
        assert_eq!(
            parse_args([
                "--items".to_owned(),
                "requirements.json".to_owned(),
                "--workers".to_owned(),
                "3".to_owned(),
            ]),
            Ok(Command::Search {
                items: PathBuf::from("requirements.json"),
                workers: NonZeroUsize::new(3),
                output: None,
                json: false,
                random_start: false,
            })
        );
        assert_eq!(
            parse_args(["-i".to_owned(), "requirements.json".to_owned()]),
            Ok(Command::Search {
                items: PathBuf::from("requirements.json"),
                workers: None,
                output: None,
                json: false,
                random_start: false,
            })
        );
    }

    #[test]
    fn items_file_can_customize_a_benchmark_query() {
        assert_eq!(
            parse_args([
                "-i".to_owned(),
                "requirements.json".to_owned(),
                "-b".to_owned(),
                "1000".to_owned(),
            ]),
            Ok(Command::Benchmark(BenchmarkOptions {
                seeds: 1_000,
                workers: None,
                items: Some(PathBuf::from("requirements.json")),
                random_start: false,
            }))
        );
    }

    #[test]
    fn accepts_json_and_text_output_files_in_either_option_order() {
        for arguments in [
            vec![
                "--items",
                "requirements.json",
                "--json",
                "--output",
                "results.json",
            ],
            vec!["-o", "results.json", "--json", "-i", "requirements.json"],
        ] {
            assert_eq!(
                parse_args(arguments.into_iter().map(str::to_owned)),
                Ok(Command::Search {
                    items: PathBuf::from("requirements.json"),
                    workers: None,
                    output: Some(PathBuf::from("results.json")),
                    json: true,
                    random_start: false,
                })
            );
        }
        assert_eq!(
            parse_args(["-i", "requirements.json", "-o", "seeds.txt"].map(str::to_owned)),
            Ok(Command::Search {
                items: PathBuf::from("requirements.json"),
                workers: None,
                output: Some(PathBuf::from("seeds.txt")),
                json: false,
                random_start: false,
            })
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_benchmark_output_options() {
        assert_eq!(
            parse_args(["-i", "requirements.json", "--json"].map(str::to_owned)),
            Err("--json requires --output FILE".to_owned())
        );
        for arguments in [
            vec!["--json"],
            vec!["--json", "--output", "results.json"],
            vec!["--output", "results.txt"],
            vec!["-i", "requirements.json", "--output"],
            vec!["-i", "requirements.json", "--output", "--json"],
            vec!["-i", "requirements.json", "--output", ""],
            vec!["-i", "requirements.json", "--json", "--output", "-"],
            vec![
                "-i",
                "requirements.json",
                "--json",
                "--json",
                "-o",
                "results.json",
            ],
            vec!["-i", "requirements.json", "--output", "one", "-o", "two"],
            vec!["--benchmark", "--output", "results.txt"],
            vec![
                "-i",
                "requirements.json",
                "-b",
                "--json",
                "-o",
                "results.json",
            ],
        ] {
            assert!(
                parse_args(arguments.iter().map(|value| (*value).to_owned())).is_err(),
                "{arguments:?}"
            );
        }
    }

    #[test]
    fn resin_only_search_loads_filters_and_exports_the_query() {
        let directory = tempfile::tempdir().unwrap();
        let items = directory.path().join("resin.json");
        let output = directory.path().join("results.json");
        std::fs::write(&items, r#"{"max_depth":1,"require_blacksmith":true,"requirements":[],"arcane_resin":65535,"arcane_resin_filter":{"uncursed":false,"max_depth":4,"source":"chest"}}"#).unwrap();
        let query = super::load_query(&items).unwrap();
        assert_eq!(query.arcane_resin, 65535);
        assert!(!query.arcane_resin_filter.uncursed);
        assert_eq!(
            query.arcane_resin_filter.source,
            Some(shpd_seedfinder_core::model::ItemSource::Chest)
        );
        super::search_command(&items, NonZeroUsize::new(1), Some(&output), true, false).unwrap();
        let imported =
            shpd_seedfinder_core::results_export::decode(&std::fs::read_to_string(output).unwrap())
                .unwrap();
        assert_eq!(imported.query, query);
        assert!(imported.seeds.is_empty());
    }

    #[test]
    fn impossible_search_creates_an_importable_empty_export() {
        let directory = tempfile::tempdir().unwrap();
        let items = directory.path().join("requirements.json");
        let output = directory.path().join("results.json");
        std::fs::write(
            &items,
            r#"{"max_depth":1,"requirements":[{"item":"ring_wealth","upgrade":4}]}"#,
        )
        .unwrap();
        super::search_command(&items, NonZeroUsize::new(1), Some(&output), true, true).unwrap();
        let imported =
            shpd_seedfinder_core::results_export::decode(&std::fs::read_to_string(output).unwrap())
                .unwrap();
        assert!(imported.seeds.is_empty());
        assert_eq!(imported.query, super::load_query(&items).unwrap());
    }

    #[test]
    fn refuses_to_overwrite_the_input_query() {
        let directory = tempfile::tempdir().unwrap();
        let items = directory.path().join("requirements.json");
        let contents = r#"{"requirements":[{"kind":"ring"}]}"#;
        std::fs::write(&items, contents).unwrap();
        for json in [false, true] {
            assert!(
                super::search_command(&items, NonZeroUsize::new(1), Some(&items), json, true)
                    .is_err()
            );
            assert_eq!(std::fs::read_to_string(&items).unwrap(), contents);
        }
    }

    #[test]
    fn rejects_unknown_or_conflicting_arguments() {
        assert_eq!(
            parse_args(["--unknown".to_owned()]),
            Err("unknown option '--unknown'".to_owned())
        );
        assert_eq!(
            parse_args(["-b".to_owned(), "--help".to_owned()]),
            Err("help and version cannot be combined with other options".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_seed_and_worker_counts() {
        for value in ["0", "many", &(TOTAL_SEEDS + 1).to_string()] {
            assert!(parse_args(["-b".to_owned(), value.to_owned()]).is_err());
        }
        for value in ["0", "many"] {
            assert!(
                parse_args(["-b".to_owned(), "--workers".to_owned(), value.to_owned(),]).is_err()
            );
        }
        assert!(parse_args(["--workers".to_owned(), "4".to_owned()]).is_err());
    }

    #[test]
    fn help_lists_benchmark_seed_and_worker_options() {
        assert!(help().contains("-b, --benchmark"));
        assert!(help().contains("-i, --items FILE"));
        assert!(help().contains("--workers WORKERS"));
        assert!(help().contains("--output FILE"));
        assert!(help().contains("--json"));
        assert!(help().contains("--random-start"));
        assert!(help().contains("default: AAA-AAA-AAA"));
        assert!(help().contains("Advance by 1"));
    }

    #[test]
    fn accepts_random_start_for_searches_exports_and_benchmarks_in_either_order() {
        for arguments in [
            vec!["--random-start", "--items", "requirements.json"],
            vec!["-i", "requirements.json", "--random-start"],
        ] {
            assert_eq!(
                parse_args(arguments.into_iter().map(str::to_owned)),
                Ok(Command::Search {
                    items: PathBuf::from("requirements.json"),
                    workers: None,
                    output: None,
                    json: false,
                    random_start: true,
                })
            );
        }
        for (arguments, json) in [
            (
                vec!["--random-start", "-i", "query.json", "-o", "results.txt"],
                false,
            ),
            (
                vec![
                    "-i",
                    "query.json",
                    "--json",
                    "-o",
                    "results.json",
                    "--random-start",
                ],
                true,
            ),
        ] {
            assert!(matches!(
                parse_args(arguments.into_iter().map(str::to_owned)),
                Ok(Command::Search { random_start: true, json: actual, .. }) if actual == json
            ));
        }
        for arguments in [
            vec!["--random-start", "-b", "16", "--workers", "2"],
            vec!["--benchmark", "16", "--workers", "2", "--random-start"],
        ] {
            assert_eq!(
                parse_args(arguments.into_iter().map(str::to_owned)),
                Ok(Command::Benchmark(BenchmarkOptions {
                    seeds: 16,
                    workers: NonZeroUsize::new(2),
                    items: None,
                    random_start: true,
                }))
            );
        }
    }

    #[test]
    fn rejects_duplicate_or_standalone_random_start() {
        for arguments in [
            vec!["--random-start"],
            vec!["--random-start", "--workers", "2"],
            vec!["--random-start", "--random-start", "-b", "16"],
            vec!["-i", "query.json", "--random-start", "--random-start"],
            vec!["--random-start", "--help"],
        ] {
            assert!(
                parse_args(arguments.iter().map(|value| (*value).to_owned())).is_err(),
                "{arguments:?}"
            );
        }
    }

    #[test]
    fn default_start_is_zero_and_random_starts_vary_between_searches() {
        assert_eq!(search_start(false), DungeonSeed::MIN);
        let starts = (0..16)
            .map(|_| search_start(true))
            .collect::<std::collections::HashSet<_>>();
        assert!(
            starts.len() > 1,
            "random searches must not always use the same start"
        );
    }

    #[test]
    fn consecutive_search_windows_wrap_and_stop_after_the_requested_count() {
        assert_eq!(
            search_ranges(DungeonSeed::MIN, 10, 4)
                .flatten()
                .collect::<Vec<_>>(),
            (0..10).collect::<Vec<_>>()
        );
        let start = DungeonSeed::new(TOTAL_SEEDS - 2).unwrap();
        assert_eq!(
            search_ranges(start, 8, 3).collect::<Vec<_>>(),
            vec![TOTAL_SEEDS - 2..TOTAL_SEEDS, 0..3, 3..6]
        );
        assert_eq!(
            search_ranges(start, 2, TOTAL_SEEDS).collect::<Vec<_>>(),
            vec![TOTAL_SEEDS - 2..TOTAL_SEEDS]
        );
        assert_eq!(
            search_ranges(start, 3, TOTAL_SEEDS)
                .flatten()
                .collect::<Vec<_>>(),
            vec![TOTAL_SEEDS - 2, TOTAL_SEEDS - 1, 0]
        );
    }

    #[test]
    fn full_search_covers_the_seed_space_once_including_seeds_before_the_start() {
        // Inspect interval bounds without generating or enumerating trillions of seeds.
        for start in [
            DungeonSeed::MIN,
            DungeonSeed::new(7).unwrap(),
            DungeonSeed::MAX,
        ] {
            let ranges = search_ranges(start, TOTAL_SEEDS, TOTAL_SEEDS).collect::<Vec<_>>();
            assert_eq!(ranges.first().unwrap().start, start.value());
            assert_eq!(
                ranges
                    .iter()
                    .map(|range| range.end - range.start)
                    .sum::<u64>(),
                TOTAL_SEEDS
            );
            let mut sorted = ranges;
            sorted.sort_unstable_by_key(|range| range.start);
            assert_eq!(sorted.first().unwrap().start, 0);
            assert_eq!(sorted.last().unwrap().end, TOTAL_SEEDS);
            assert!(sorted.windows(2).all(|pair| pair[0].end == pair[1].start));
        }
    }
}
