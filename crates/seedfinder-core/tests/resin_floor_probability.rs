//! Integration with the floor, source-count, and duplicate-wand calibrations.
use shpd_seedfinder_core::{
    catalog::ItemKind, floor_filters::FloorRequirement, json_query,
    probability::estimate_match_probability as probability, query::SearchQuery,
};

fn floors() -> Vec<FloorRequirement> {
    json_query::decode(
        r#"{"requirements":[],"floor_requirements":[
        {"depth":7,"feeling":"dark","any_rooms":["garden","secret_garden"]},
        {"depth":17,"feeling":"dark","any_rooms":["garden","secret_garden"]}
    ]}"#,
    )
    .unwrap()
    .floor_requirements
}

fn close(actual: f64, expected: f64, query: &SearchQuery) {
    assert!(actual.is_finite() && expected.is_finite());
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs().max(1e-6),
        "{query:?}: {actual} != {expected}"
    );
}

#[test]
fn excluding_auto_cost_does_not_change_calibrated_wand_supply() {
    for document in [
        r#"{"max_depth":16,"requirements":[{"item":"wand_lightning","uncursed":true},{"item":"wand_lightning"},{"item":"wand_lightning"}]}"#,
        r#"{"requirements":[{"item":"wand_frost","source":"wandmaker_reward","upgrade":{"at_least":2}},{"item":"wand_frost"},{"item":"wand_frost"}]}"#,
        r#"{"max_depth":9,"requirements":[{"kind":"wand","source":"chest"},{"kind":"wand","source":"chest"}]}"#,
    ] {
        for trinket in [None, Some("mimic_tooth"), Some("mossy_clump")] {
            let mut query = json_query::decode(document).unwrap();
            if let Some(trinket) = trinket {
                query.requirements.extend(
                    json_query::decode(&format!(
                        r#"{{"requirements":[{{"item":"{trinket}","select_trinket":true}}]}}"#
                    ))
                    .unwrap()
                    .requirements,
                );
            }
            query.floor_requirements = floors()
                .into_iter()
                .filter(|f| f.depth <= query.max_depth)
                .collect();
            let expected = probability(&query);
            assert!(expected > 0.0);
            for r in &mut query.requirements {
                r.exclude_resin = r.kind == ItemKind::Wand;
            }
            query.validate().unwrap();
            close(probability(&query), expected, &query);
            query.arcane_resin_auto = true;
            close(probability(&query), expected, &query);
            query.arcane_resin_filter.include_mage_wand = true;
            close(probability(&query), expected, &query);
        }
    }
}

#[test]
fn farming_probability_is_applied_once_with_resin_options_and_blankets() {
    let mut floor_only = json_query::decode_unvalidated(r#"{"requirements":[]}"#).unwrap();
    floor_only.floor_requirements = floors();
    let factor = probability(&floor_only);
    assert!(factor > 0.0 && factor < 1.0);
    for document in [
        r#"{"arcane_resin":2,"arcane_resin_filter":{"include_mage_wand":true},"requirements":[]}"#,
        r#"{"arcane_resin":4,"arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"item":"wand_frost"},{"item":"wand_frost"}]}"#,
        r#"{"arcane_resin":"auto","arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"kind":"wand","identity_group":1,"exclude_resin":true},{"kind":"wand","identity_group":1}]}"#,
        r#"{"arcane_resin":"auto","arcane_resin_filter":{"include_mage_wand":true},"requirements":[{"item":"wand_lightning","exclude_resin":true},{"kind":"wand"},{"kind":"wand","source":"wandmaker_reward","blanket":true}]}"#,
    ] {
        let mut query = json_query::decode(document).unwrap();
        let baseline = probability(&query);
        assert!(baseline > 0.0);
        query.floor_requirements = floors();
        close(probability(&query), baseline * factor, &query);
        // The same resin cache entry must remain valid when floor filters change.
        query.floor_requirements.reverse();
        close(probability(&query), baseline * factor, &query);
        query.floor_requirements.clear();
        close(probability(&query), baseline, &query);
    }
}

#[test]
fn fixed_auto_cost_keeps_repeat_calibration_and_credit_equivalence() {
    for source in ["any", "wandmaker_reward"] {
        for upgrade in [1, 2, 3] {
            let mut auto = json_query::decode(&format!(
                r#"{{
                "arcane_resin":"auto","arcane_resin_filter":{{"include_mage_wand":true}},
                "requirements":[{{"item":"wand_frost","upgrade":{upgrade},"exclude_resin":true}},
                                {{"item":"wand_frost","upgrade":{upgrade}}}]
            }}"#
            ))
            .unwrap();
            if source != "any" {
                auto.requirements[0].source =
                    Some(shpd_seedfinder_core::model::ItemSource::WandmakerReward);
            }
            auto.floor_requirements = floors();
            let mut fixed = auto.clone();
            fixed.arcane_resin_auto = false;
            fixed.arcane_resin = match upgrade {
                1 => 5,
                2 => 3,
                _ => 0,
            };
            close(probability(&auto), probability(&fixed), &auto);
            fixed.arcane_resin = fixed.arcane_resin.saturating_sub(2);
            fixed.arcane_resin_filter.include_mage_wand = false;
            close(probability(&auto), probability(&fixed), &auto);
        }
    }
}

#[test]
fn credits_and_exclusions_relax_auto_without_exceeding_calibrated_baselines() {
    for document in [
        r#"{"requirements":[{"item":"wand_frost"},{"item":"wand_frost"}]}"#,
        r#"{"requirements":[{"kind":"wand","identity_group":1,"uncursed":true},{"kind":"wand","identity_group":1},{"kind":"wand","identity_group":1}]}"#,
    ] {
        for auto_trinket in [false, true] {
            let mut query = json_query::decode(document).unwrap();
            query.auto_apply_trinket = auto_trinket;
            query.floor_requirements = floors();
            let baseline = probability(&query);
            query.arcane_resin_auto = true;
            let strict = probability(&query);
            query.arcane_resin_filter.include_mage_wand = true;
            let credit = probability(&query);
            query.requirements[0].exclude_resin = true;
            let excluded = probability(&query);
            assert!(strict.is_finite() && credit.is_finite() && excluded.is_finite());
            assert!(strict > 0.0 && strict <= credit + 1e-12);
            assert!(
                credit <= excluded + 1e-12 && excluded <= baseline + 1e-12,
                "{query:?}: strict={strict}, credit={credit}, excluded={excluded}, baseline={baseline}"
            );
        }
    }
}
