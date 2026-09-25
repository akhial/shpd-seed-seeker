#![cfg(unix)]

use std::fs;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use shpd_seedfinder_core::results_export;
use shpd_seedfinder_core::seed::DungeonSeed;

// Always reap the subprocess, including on assertion failures.
struct SearchProcess(Option<Child>);

impl Drop for SearchProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "search did not respond in time");
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn shutdown_signals_flush_results_and_report_session_statistics() {
    for (signal, json) in [
        ("-INT", false),
        ("-TERM", false),
        ("-HUP", false),
        ("-INT", true),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let items = directory.path().join("requirements.json");
        let output = directory.path().join("results");
        fs::write(
            &items,
            r#"{"max_depth":1,"requirements":[{"item":"ring_wealth"}]}"#,
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_seed-seeker"));
        command
            .arg("--items")
            .arg(&items)
            .args(["--workers", "2", "--output"])
            .arg(&output)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if json {
            command.arg("--json");
        }
        let mut process = SearchProcess(Some(command.spawn().unwrap()));

        // Creating the output proves that the shutdown handler is installed.
        // For JSON, also wait for a live snapshot to exercise preserving it.
        let mut published = Vec::new();
        wait_until(|| {
            if json {
                if let Some(document) = fs::read_to_string(&output)
                    .ok()
                    .and_then(|contents| results_export::decode(&contents).ok())
                {
                    published = document.seeds;
                }
                !published.is_empty()
            } else {
                output.exists()
            }
        });
        let child = process.0.as_mut().unwrap();
        assert!(child.try_wait().unwrap().is_none());
        assert!(
            Command::new("kill")
                .args([signal, &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        wait_until(|| child.try_wait().unwrap().is_some());
        let result = process.0.take().unwrap().wait_with_output().unwrap();
        let report = String::from_utf8(result.stderr).unwrap();
        assert!(result.status.success(), "{signal}: {report}");
        assert!(result.stdout.is_empty(), "statistics must stay off stdout");

        let contents = fs::read_to_string(&output).unwrap();
        let seeds = if json {
            results_export::decode(&contents).unwrap().seeds
        } else {
            contents
                .lines()
                .map(|line| DungeonSeed::from_code(line).unwrap())
                .collect::<Vec<_>>()
        };
        assert!(published.iter().all(|seed| seeds.contains(seed)));
        assert_eq!(report.matches("Search session statistics").count(), 1);
        assert_eq!(statistic::<usize>(&report, "Matches found: "), seeds.len());
        assert!(statistic::<usize>(&report, "Seeds searched: ") >= seeds.len());
        assert!(statistic::<f64>(&report, "Time spent: ").is_finite());
        assert!(statistic::<f64>(&report, "Average seeds/second: ").is_finite());
    }
}

fn statistic<T: std::str::FromStr>(report: &str, label: &str) -> T {
    report
        .lines()
        .find_map(|line| line.strip_prefix(label))
        .unwrap_or_else(|| panic!("missing {label} in {report}"))
        .trim_end_matches(" s")
        .parse()
        .unwrap_or_else(|_| panic!("invalid {label} in {report}"))
}
