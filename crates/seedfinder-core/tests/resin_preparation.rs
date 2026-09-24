//! Public probability/ranking regressions recorded before transition reuse
//! (main 8eeeb63c). Includes the reported slow query, interacting blankets,
//! reservation replacements, fixed budgets, donor filters and explicit profiles.
use shpd_seedfinder_core::{
    auto_trinkets::AutoTrinketPolicy, catalog::item, json_query,
    probability::estimate_match_probability,
};

#[test]
fn preparation_preserves_resin_probabilities_and_trinket_rankings() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/resin-preparation.json")).unwrap();
    for case in cases {
        let query = json_query::decode(&case["query"].to_string()).unwrap();
        let expected = case["probability"].as_f64().unwrap();
        let actual = estimate_match_probability(&query);
        // Permit platform libm rounding without tolerating changes in the model,
        // including the very small probabilities of the reported mixed queries.
        assert!(
            (actual - expected).abs() <= expected.abs() * 1e-12,
            "{}: {actual} != {expected}",
            case["name"],
        );
        let preferred = AutoTrinketPolicy::prepare(&query).map(|policy| {
            policy
                .preferred()
                .iter()
                .map(|&id| item(id).stable_id)
                .collect::<Vec<_>>()
        });
        assert_eq!(
            serde_json::json!(preferred),
            case["preferred"],
            "{}",
            case["name"]
        );
    }
}
