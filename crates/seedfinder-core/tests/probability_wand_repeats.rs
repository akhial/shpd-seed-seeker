//! Independent observations cover linked/named copies, uneven deadlines,
//! upgrades, curses, and reward sources. No calibration seeds are reused.
use shpd_seedfinder_core::{json_query, probability::estimate_match_probability};

#[test]
fn duplicate_wands_track_independent_observations() {
    let mut compared = 0;
    let mut reports: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/probability-wand-profiles.json")).unwrap();
    for document in [
        include_str!("fixtures/probability-wand-sweep.json"),
        include_str!("fixtures/probability-wand-rewards.json"),
        include_str!("fixtures/probability-wand-random.json"),
    ] {
        reports.push(serde_json::from_str(document).unwrap());
    }
    for report in reports {
        let samples = report["samples"].as_f64().unwrap();
        for case in report["cases"].as_array().unwrap() {
            let query = json_query::decode(&case["query"].to_string()).unwrap();
            let estimate = estimate_match_probability(&query);
            let hits = case["hits"].as_f64().unwrap();
            assert!(estimate.is_finite() && (0.0..=1.0).contains(&estimate));
            if hits.max(estimate * samples) < 30.0 {
                continue;
            }
            // Same four-sigma Wilson interval and 1.35 factor as the general
            // audit. Zero-hit overestimates count; sparse cases do not pass.
            let observed = hits / samples;
            let center = (observed + 8.0 / samples) / (1.0 + 16.0 / samples);
            let half = 4.0
                * (observed * (1.0 - observed) / samples + 4.0 / (samples * samples)).sqrt()
                / (1.0 + 16.0 / samples);
            assert!(
                estimate >= (center - half) / 1.35 && estimate <= ((center + half) * 1.35).min(1.0),
                "{}: predicted {}, observed {hits}/{samples}",
                case["query"],
                estimate * samples
            );
            compared += 1;
        }
    }
    assert!(compared > 500, "only {compared} sufficiently sampled cases");
}
