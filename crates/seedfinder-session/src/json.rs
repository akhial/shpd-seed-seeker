//! The packet-shaped UTF-8 envelopes the native bridges (C, JNI) hand their
//! frontends, built in one place so both platforms read identical documents.
//! Envelopes that need no session state — the results codec and seed
//! parsing — live beside their codecs in `seedfinder-core`
//! (`results_export::encode_document`/`decode_document`,
//! `seed::parse_document`), where the browser bridge reaches them too.
//!
//! Each bridge function reduces to marshalling its platform's bytes and one
//! call here; the shapes below are the contract the frontends parse.

use serde_json::json;

use crate::{ScoutMatchError, production_scout_matches};

/// Marks which items of the world named by an `SSQ2` (or legacy raw seed)
/// scout request satisfy the query, as `{"matched": [<item indices>],
/// "matchedRequirements": <n>, "totalRequirements": <n>}`. The indices
/// address the item list of the `SSC5` packet the same request scouts to.
/// `transmutedTrinkets` contains separate zero-based indices into the 13-card
/// tail. It never changes the generated item indices; older clients ignore it.
/// The keys are camelCase like every other bridge-built document (the
/// browser's own scout output and `engine_info`); only the persisted formats
/// — query documents and results files — are `snake_case`.
///
/// # Errors
///
/// Returns [`production_scout_matches`]'s error.
pub fn scout_matches_document(request: &[u8], query: &[u8]) -> Result<String, ScoutMatchError> {
    let marks = production_scout_matches(request, query)?;
    Ok(json!({
        "matched": marks.matched_indices(),
        "transmutedTrinkets": marks.transmuted_trinkets.iter().enumerate()
            .filter_map(|(index, &matched)| matched.then_some(index)).collect::<Vec<_>>(),
        "matchedRequirements": marks.matched_requirements,
        "totalRequirements": marks.total_requirements,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use shpd_seedfinder_core::catalog::item;
    use shpd_seedfinder_core::challenges::Challenges;
    use shpd_seedfinder_core::json_query;
    use shpd_seedfinder_core::seed::DungeonSeed;
    use shpd_seedfinder_core::wire::decode_scout_world;

    use super::*;
    use crate::{production_scout_packet, production_scout_world};

    /// The request bytes a frontend sends for a query: its canonical JSON
    /// document.
    fn query_request(query: &shpd_seedfinder_core::query::SearchQuery) -> Vec<u8> {
        json_query::encode(query).to_string().into_bytes()
    }

    #[test]
    fn transmutation_marks_are_separate_from_native_item_indices() {
        let query = br#"{"requirements":[{"item":"rat_skull","trinket_transmutations":11}]}"#;
        let envelope: Value =
            serde_json::from_str(&scout_matches_document(b"AAA-AAA-AAA", query).unwrap()).unwrap();
        assert_eq!(envelope["matched"], json!([]));
        assert_eq!(envelope["transmutedTrinkets"], json!([10]));
        assert_eq!(envelope["matchedRequirements"], 1);
    }

    #[test]
    fn scout_match_envelope_indexes_the_scout_packet() {
        // Scouting is deterministic, so the marks index exactly the item list
        // the SSC5 packet of the same request carries.
        let seed = DungeonSeed::MIN;
        let world = production_scout_world(seed, Challenges::NONE).unwrap();
        let known = &world.items[0];
        let document = json!({
            "requirements": [{
                "item": item(known.item).stable_id,
                "max_depth": known.depth,
            }],
        });
        let query = query_request(&json_query::decode(&document.to_string()).unwrap());

        let envelope: Value =
            serde_json::from_str(&scout_matches_document(b"AAA-AAA-AAA", &query).unwrap()).unwrap();
        assert_eq!(envelope["totalRequirements"], 1);
        assert_eq!(envelope["matchedRequirements"], 1);
        let matched = envelope["matched"].as_array().unwrap();
        assert_eq!(matched.len(), 1);
        let index = usize::try_from(matched[0].as_u64().unwrap()).unwrap();
        let packet = production_scout_packet(b"AAA-AAA-AAA").unwrap();
        let scouted = decode_scout_world(&packet).unwrap();
        assert!(index < scouted.items.len());
        assert_eq!(scouted.items[index].item, known.item);

        // An unsatisfiable requirement still reports the requirement count.
        let impossible = query_request(
            &json_query::decode(
                r#"{"requirements":[{"item":"sword","max_depth":1}],"max_depth":1}"#,
            )
            .unwrap(),
        );
        let envelope: Value =
            serde_json::from_str(&scout_matches_document(b"AAA-AAA-AAA", &impossible).unwrap())
                .unwrap();
        assert_eq!(envelope["totalRequirements"], 1);
        assert!(envelope["matched"].as_array().unwrap().len() <= 1);

        assert!(scout_matches_document(b"AAA-AAA-AA0", &query).is_err());
        assert!(scout_matches_document(b"AAA-AAA-AAA", b"bad").is_err());
    }
}
