use std::env;
use std::num::NonZeroUsize;
use std::process::ExitCode;

use shpd_seedfinder_core::SHPD_VERSION;
use shpd_seedfinder_core::catalog::{ItemId, ItemKind};
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::query::{Requirement, SearchQuery, UpgradeRequirement};
use shpd_seedfinder_core::search::{SearchOptions, SearchProgress, search_parallel};
use shpd_seedfinder_core::seed::TOTAL_SEEDS;

const DEFAULT_BENCHMARK_SEEDS: u64 = 10_000;
const SEARCH_CHUNK_SIZE: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BenchmarkOptions {
    seeds: u64,
    workers: Option<NonZeroUsize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Benchmark(BenchmarkOptions),
    Help,
    Version,
}

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Benchmark(options)) => match run_benchmark(options) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("seed-seeker: benchmark failed: {error}");
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
    if arguments.is_empty() {
        return Ok(Command::Help);
    }
    if arguments.len() == 1 {
        match arguments[0].as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            _ => {}
        }
    }

    let mut benchmark = false;
    let mut seeds = DEFAULT_BENCHMARK_SEEDS;
    let mut workers = None;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--benchmark" | "-b" => {
                if benchmark {
                    return Err("benchmark may only be specified once".to_owned());
                }
                benchmark = true;
                if arguments
                    .get(index + 1)
                    .is_some_and(|argument| !argument.starts_with('-'))
                {
                    index += 1;
                    seeds = parse_seed_count(&arguments[index])?;
                }
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
            "--help" | "-h" | "--version" | "-V" => {
                return Err("help and version cannot be combined with a benchmark".to_owned());
            }
            argument => return Err(format!("unknown option '{argument}'")),
        }
        index += 1;
    }

    if !benchmark {
        return Err("--workers requires --benchmark".to_owned());
    }
    Ok(Command::Benchmark(BenchmarkOptions { seeds, workers }))
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
        "Usage: seed-seeker --benchmark [SEEDS] [--workers WORKERS]\n\n",
        "Options:\n",
        "  -b, --benchmark [SEEDS]  Benchmark the canonical depth-24 seed search\n",
        "                            [default: 10000]\n",
        "      --workers WORKERS     Number of search workers [default: available CPUs]\n",
        "  -h, --help                Print help\n",
        "  -V, --version             Print version\n",
    )
}

fn run_benchmark(benchmark: BenchmarkOptions) -> Result<String, String> {
    let workers = benchmark
        .workers
        .unwrap_or_else(SearchOptions::available_parallelism);
    let query = benchmark_query();
    let options = SearchOptions {
        start_seed: 0,
        end_seed_exclusive: benchmark.seeds,
        workers,
        chunk_size: NonZeroUsize::new(SEARCH_CHUNK_SIZE).expect("chunk size is non-zero"),
        max_results: NonZeroUsize::MAX,
    };
    let outcome = search_parallel(
        &CanonicalMainWorldGenerator,
        &query,
        options,
        &SearchProgress::default(),
    )
    .map_err(|error| format!("{error:?}"))?;

    Ok(format!(
        concat!(
            "Seed Seeker benchmark (Shattered Pixel Dungeon {shpd_version})\n",
            "Workers: {workers}\n",
            "Seeds tested: {tested}\n",
            "Elapsed: {elapsed:.3} s\n",
            "Throughput: {throughput:.0} seeds/s",
        ),
        shpd_version = SHPD_VERSION,
        workers = workers,
        tested = outcome.tested,
        elapsed = outcome.elapsed.as_secs_f64(),
        throughput = outcome.seeds_per_second(),
    ))
}

fn benchmark_query() -> SearchQuery {
    SearchQuery {
        requirements: vec![Requirement {
            kind: ItemKind::Ring,
            item: Some(ItemId::RingTenacity),
            upgrade: UpgradeRequirement::Exact(4),
            effect: None,
            source: None,
            identity_group: None,
        }],
        max_depth: 24,
        require_blacksmith: false,
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use shpd_seedfinder_core::seed::TOTAL_SEEDS;

    use super::{BenchmarkOptions, Command, help, parse_args};

    fn benchmark(seeds: u64, workers: Option<usize>) -> Command {
        Command::Benchmark(BenchmarkOptions {
            seeds,
            workers: workers.and_then(NonZeroUsize::new),
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
    fn rejects_unknown_or_conflicting_arguments() {
        assert_eq!(
            parse_args(["--unknown".to_owned()]),
            Err("unknown option '--unknown'".to_owned())
        );
        assert_eq!(
            parse_args(["-b".to_owned(), "--help".to_owned()]),
            Err("help and version cannot be combined with a benchmark".to_owned())
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
        assert!(help().contains("--workers WORKERS"));
    }
}
