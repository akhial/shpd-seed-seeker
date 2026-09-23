//! Live, cross-platform results exports. The published path always contains
//! a complete document; updates are prepared beside it and renamed into place.

use std::fs;
use std::io::{self, Write as _};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use shpd_seedfinder_core::feasibility::QueryPlan;
use shpd_seedfinder_core::main_world::CanonicalMainWorldGenerator;
use shpd_seedfinder_core::query::SearchQuery;
use shpd_seedfinder_core::results_export::{self, MAX_FILE_BYTES, MAX_RESULTS};
use shpd_seedfinder_core::search::{
    SearchOptions, StreamingSearchHandle, StreamingSearchState, spawn_streaming_search,
};
use shpd_seedfinder_core::seed::{DungeonSeed, TOTAL_SEEDS};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn search(
    query: &SearchQuery,
    workers: NonZeroUsize,
    path: &Path,
) -> Result<(), String> {
    let mut output = JsonOutput::new(path, query)?;
    if QueryPlan::analyze(query).is_unsatisfiable() {
        eprintln!(
            "seed-seeker: no seed can satisfy this query within depth {}; nothing to search",
            query.max_depth
        );
        return Ok(());
    }
    let handle = spawn_streaming_search(
        &Arc::new(CanonicalMainWorldGenerator::with_challenges(
            query.challenges,
        )),
        query.clone(),
        SearchOptions {
            start_seed: 0,
            end_seed_exclusive: TOTAL_SEEDS,
            workers,
            chunk_size: NonZeroUsize::new(super::SEARCH_CHUNK_SIZE)
                .expect("chunk size is non-zero"),
            max_results: NonZeroUsize::new(MAX_RESULTS).expect("result limit is non-zero"),
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    stream_results(&handle, &mut output)
}

fn stream_results(
    handle: &StreamingSearchHandle,
    output: &mut JsonOutput<'_>,
) -> Result<(), String> {
    loop {
        let worlds = handle.drain_results(MAX_RESULTS - output.seeds.len());
        if !worlds.is_empty() {
            output.append(worlds.into_iter().map(|world| world.seed))?;
        }
        if output.seeds.len() == MAX_RESULTS {
            eprintln!("seed-seeker: exported {MAX_RESULTS} matches; reached the app result limit");
            return Ok(());
        }
        match handle.state() {
            StreamingSearchState::Running => std::thread::sleep(POLL_INTERVAL),
            StreamingSearchState::Completed | StreamingSearchState::Cancelled => return Ok(()),
            StreamingSearchState::Failed => {
                return Err(format!("search worker failed: {:?}", handle.failure()));
            }
        }
    }
}

struct JsonOutput<'a> {
    path: &'a Path,
    query: &'a SearchQuery,
    seeds: Vec<DungeonSeed>,
}

impl<'a> JsonOutput<'a> {
    fn new(path: &'a Path, query: &'a SearchQuery) -> Result<Self, String> {
        if fs::metadata(path).is_ok_and(|metadata| !metadata.is_file()) {
            return Err(format!(
                "JSON output '{}' must be a regular file",
                path.display()
            ));
        }
        let output = Self {
            path,
            query,
            seeds: Vec::new(),
        };
        let contents = results_export::encode(query, &[], env!("CARGO_PKG_VERSION"));
        // The CLI query format can express groups the app editors cannot.
        // Reject those before replacing an existing output or starting work.
        results_export::decode(&contents)
            .map_err(|error| format!("query cannot be exported for app import: {error}"))?;
        output.publish(&contents)?;
        Ok(output)
    }

    fn append(&mut self, seeds: impl IntoIterator<Item = DungeonSeed>) -> Result<(), String> {
        self.seeds
            .extend(seeds.into_iter().take(MAX_RESULTS - self.seeds.len()));
        // Reuse the canonical codec, including the query and selected trinkets.
        let contents = results_export::encode(self.query, &self.seeds, env!("CARGO_PKG_VERSION"));
        self.publish(&contents)
    }

    fn publish(&self, contents: &str) -> Result<(), String> {
        if contents.len() > MAX_FILE_BYTES {
            return Err(
                "JSON output would exceed the 2 MiB app import limit; previous export preserved"
                    .to_owned(),
            );
        }
        write_atomically(self.path, contents.as_bytes()).map_err(|error| {
            format!(
                "could not write JSON output '{}': {error}",
                self.path.display()
            )
        })
    }
}

fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".seed-seeker-")
        .tempfile_in(parent)?;
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    // Clear Windows' temporary-file attribute before publishing, then close
    // the writer. `keep` also disables automatic cleanup, so handle it below.
    let (file, temporary_path) = temporary.keep().map_err(|error| error.error)?;
    drop(file);
    // Unlike tempfile::persist's MoveFileExW, std's rename can replace a file
    // with open readers on modern Windows. Keep the same-directory replacement
    // atomic: never truncate or remove the published file first.
    fs::rename(&temporary_path, path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary_path);
    })
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;
    use std::sync::{Mutex, mpsc};
    use std::time::Instant;

    use shpd_seedfinder_core::catalog::ItemId;
    use shpd_seedfinder_core::json_query;
    use shpd_seedfinder_core::model::{Accessibility, GeneratedWorld, ItemSource, WorldItem};
    use shpd_seedfinder_core::quests::QuestSummary;
    use shpd_seedfinder_core::run::RingGems;
    use shpd_seedfinder_core::search::WorldGenerator;

    use super::*;

    fn query() -> SearchQuery {
        json_query::decode(
            r#"{"max_depth":19,"auto_apply_trinket":true,"requirements":[{"item":"runic_blade","effect":"Grim"}]}"#,
        )
        .unwrap()
    }

    #[test]
    fn every_update_imports_with_the_query_versions_and_trinket_choices() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        let query = query();
        let mut output = JsonOutput::new(&path, &query).unwrap();
        let seeds =
            ["SRU-YSU-QHS", "EYY-RUL-LQG"].map(|code| DungeonSeed::from_code(code).unwrap());

        for count in 0..=seeds.len() {
            if count > 0 {
                output.append([seeds[count - 1]]).unwrap();
            }
            let contents = fs::read_to_string(&path).unwrap();
            let imported = results_export::decode(&contents).unwrap();
            assert_eq!(imported.query, query);
            assert_eq!(imported.seeds, seeds[..count]);
            assert_eq!(
                imported.app_version.as_deref(),
                Some(env!("CARGO_PKG_VERSION"))
            );
            assert_eq!(
                imported.shpd_version.as_deref(),
                Some(shpd_seedfinder_core::SHPD_VERSION)
            );
            assert_eq!(
                contents,
                results_export::encode(&query, &seeds[..count], env!("CARGO_PKG_VERSION"))
            );
            assert!(
                imported
                    .recipes
                    .iter()
                    .all(|recipe| recipe.trinket.is_some())
            );
            // This is the import entry point used by the native/web bridges.
            results_export::decode_document(&contents).unwrap();
        }
    }

    #[test]
    fn readers_keep_a_complete_snapshot_across_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        let query = query();
        let mut output = JsonOutput::new(&path, &query).unwrap();
        let mut previous = fs::File::open(&path).unwrap();
        output.append([DungeonSeed::MIN]).unwrap();

        let mut contents = String::new();
        previous.read_to_string(&mut contents).unwrap();
        assert!(results_export::decode(&contents).unwrap().seeds.is_empty());
        assert_eq!(
            results_export::decode(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .seeds,
            [DungeonSeed::MIN]
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replacement_preserves_destination_and_cleans_up_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        fs::create_dir(&path).unwrap();
        let existing = path.join("existing");
        fs::write(&existing, "preserved").unwrap();

        assert!(write_atomically(&path, b"replacement").is_err());

        assert_eq!(fs::read_to_string(&existing).unwrap(), "preserved");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn locked_destination_preserves_export_and_cleans_up_temporary_file() {
        use std::os::windows::fs::OpenOptionsExt as _;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        write_atomically(&path, b"previous export").unwrap();
        let mut reader = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .unwrap();

        assert!(write_atomically(&path, b"replacement").is_err());

        let mut contents = String::new();
        reader.read_to_string(&mut contents).unwrap();
        assert_eq!(contents, "previous export");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        drop(reader);
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous export");
        write_atomically(&path, b"replacement").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
    }

    #[test]
    fn streaming_search_stops_at_the_import_limit_with_multiple_workers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        let query = json_query::decode(r#"{"requirements":[{"item":"ring_wealth"}]}"#).unwrap();
        let mut output = JsonOutput::new(&path, &query).unwrap();
        let handle = spawn_streaming_search(
            &Arc::new(MatchingGenerator),
            query.clone(),
            SearchOptions {
                start_seed: 0,
                end_seed_exclusive: TOTAL_SEEDS,
                workers: NonZeroUsize::new(4).unwrap(),
                chunk_size: NonZeroUsize::new(17).unwrap(),
                max_results: NonZeroUsize::new(MAX_RESULTS).unwrap(),
            },
        )
        .unwrap();
        stream_results(&handle, &mut output).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        let imported = results_export::decode(&contents).unwrap();
        assert_eq!(imported.seeds.len(), MAX_RESULTS);
        assert_eq!(
            results_export::dedupe_and_cap(&imported.seeds, MAX_RESULTS).1,
            0
        );
    }

    struct MatchingGenerator;

    impl WorldGenerator for MatchingGenerator {
        fn generate(&self, seed: DungeonSeed, _: u8) -> GeneratedWorld {
            GeneratedWorld {
                seed,
                items: vec![WorldItem {
                    item: ItemId::RingWealth,
                    upgrade: 0,
                    effect: None,
                    cursed: false,
                    depth: 1,
                    source: ItemSource::Heap,
                    accessibility: Accessibility::Independent,
                    secret: false,
                }],
                floor_rooms: Vec::new(),
                feelings: Vec::new(),
                quests: QuestSummary::default(),
                ring_gems: RingGems::UNSHUFFLED,
            }
        }
    }

    #[test]
    fn publishes_matches_before_search_completion_and_preserves_them_on_cancellation() {
        struct PausingGenerator(Mutex<mpsc::Receiver<()>>);

        impl WorldGenerator for PausingGenerator {
            fn generate(&self, seed: DungeonSeed, depth: u8) -> GeneratedWorld {
                if seed != DungeonSeed::MIN {
                    self.0
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap();
                }
                MatchingGenerator.generate(seed, depth)
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        let query = json_query::decode(r#"{"requirements":[{"item":"ring_wealth"}]}"#).unwrap();
        let mut output = JsonOutput::new(&path, &query).unwrap();
        let (resume, receiver) = mpsc::channel();
        let handle = spawn_streaming_search(
            &Arc::new(PausingGenerator(Mutex::new(receiver))),
            query.clone(),
            SearchOptions {
                start_seed: 0,
                end_seed_exclusive: 2,
                workers: NonZeroUsize::MIN,
                chunk_size: NonZeroUsize::MIN,
                max_results: NonZeroUsize::new(MAX_RESULTS).unwrap(),
            },
        )
        .unwrap();

        std::thread::scope(|scope| {
            let writer = scope.spawn(|| stream_results(&handle, &mut output));
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let imported = results_export::decode(&fs::read_to_string(&path).unwrap()).unwrap();
                if !imported.seeds.is_empty() {
                    assert_eq!(imported.seeds, [DungeonSeed::MIN]);
                    assert_eq!(handle.state(), StreamingSearchState::Running);
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "first match was not published during the search"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            handle.cancel();
            resume.send(()).unwrap();
            writer.join().unwrap().unwrap();
        });
        assert_eq!(
            results_export::decode(&fs::read_to_string(&path).unwrap())
                .unwrap()
                .seeds,
            [DungeonSeed::MIN]
        );
    }

    #[test]
    fn failed_update_preserves_the_previous_export() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        let query = query();
        let output = JsonOutput::new(&path, &query).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(output.publish(&" ".repeat(MAX_FILE_BYTES + 1)).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn rejects_queries_the_apps_cannot_import_before_replacing_output() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("results.json");
        fs::write(&path, "previous export").unwrap();
        let query =
            json_query::decode(r#"{"requirements":[{"kind":"ring","identity_group":5}]}"#).unwrap();
        assert!(JsonOutput::new(&path, &query).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "previous export");
    }
}
