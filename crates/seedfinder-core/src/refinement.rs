//! Execution under an immutable parent search's trinket choices.
use crate::auto_trinkets::{self, TrinketSearchMatch};
use crate::feasibility::QueryPlan;
use crate::query::SearchQuery;
use crate::search::WorldGenerator;
use crate::seed::DungeonSeed;

/// Prepared once and shared by workers. The original query is retained through
/// every refinement, including its removal of unnecessary automatic trinkets.
pub struct PreservedSearch {
    query: SearchQuery,
    plan: QueryPlan,
    base_plan: QueryPlan,
}

impl PreservedSearch {
    #[must_use]
    pub fn new(query: SearchQuery, base: &SearchQuery) -> Self {
        Self {
            plan: QueryPlan::analyze(&query),
            base_plan: QueryPlan::analyze(base),
            query,
        }
    }

    /// The same result as searching the parent and refining its recipes,
    /// reapplying its choice before testing, then cleaning successful matches.
    pub fn search_batch<G: WorldGenerator>(
        &self,
        generator: &G,
        seeds: &[DungeonSeed],
    ) -> Vec<Option<TrinketSearchMatch>> {
        auto_trinkets::search_batch_with_selection(
            generator,
            &self.query,
            &self.plan,
            &self.base_plan,
            seeds,
        )
    }
}

/// Search execution packets wrap a normal query and its immutable selection
/// source. This context is separate from share links and exported query JSON.
///
/// # Errors
/// Rejects malformed queries/envelopes and queries that cannot refine the base.
#[cfg(feature = "json-query")]
pub fn decode_execution(
    packet: &[u8],
) -> Result<(SearchQuery, Option<SearchQuery>), crate::wire::WireError> {
    use crate::wire::{WireError, decode_query};
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Execution {
        query: serde_json::Value,
        refine_base: serde_json::Value,
    }
    let bytes = packet.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(packet);
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return decode_query(packet).map(|q| (q, None));
    };
    if value.get("refine_base").is_none() {
        return decode_query(packet).map(|q| (q, None));
    }
    let execution: Execution = serde_json::from_value(value)
        .map_err(|e| WireError::InvalidQueryDocument(e.to_string()))?;
    let query = decode_query(execution.query.to_string().as_bytes())?;
    let base = decode_query(execution.refine_base.to_string().as_bytes())?;
    if !query.refines(&base) {
        return Err(WireError::InvalidQueryDocument(
            "The query cannot reuse this search's trinket policy and coverage".into(),
        ));
    }
    Ok((query, Some(base)))
}

#[cfg(all(test, feature = "json-query"))]
mod tests {
    use super::*;
    use crate::{catalog::ItemId, main_world::CanonicalMainWorldGenerator};

    #[test]
    fn preserved_scan_agrees_with_parent_search_then_refinement() {
        let base = crate::json_query::decode(r#"{"auto_apply_trinket":true,"max_depth":19,"requirements":[{"item":"runic_blade","upgrade":1,"effect":"Grim"}]}"#).unwrap();
        let mut query = base.clone();
        query.requirements.extend(
            crate::json_query::decode(r#"{"requirements":[{"item":"whip","effect":"Venomous"}]}"#)
                .unwrap()
                .requirements,
        );
        let seeds = [
            DungeonSeed::from_code("EYY-RUL-LQG").unwrap(),
            DungeonSeed::MIN,
        ];
        let generator = CanonicalMainWorldGenerator;
        let parent =
            auto_trinkets::search_batch(&generator, &base, &QueryPlan::analyze(&base), &seeds);
        assert_eq!(parent[0].as_ref().unwrap().recipe.trinket, None);
        let results = PreservedSearch::new(query.clone(), &base).search_batch(&generator, &seeds);
        assert_eq!(
            results[0].as_ref().unwrap().recipe.trinket,
            Some(ItemId::ParchmentScrap)
        );
        for (parent, result) in parent.into_iter().zip(results) {
            let expected = parent.and_then(|parent| {
                auto_trinkets::refine_batch(
                    &generator,
                    &query,
                    &QueryPlan::analyze(&query),
                    &base,
                    &[parent.recipe],
                )
                .pop()
                .flatten()
            });
            assert_eq!(
                result.as_ref().map(|m| (&m.recipe, &m.world)),
                expected.as_ref().map(|m| (&m.recipe, &m.world))
            );
        }
    }

    #[test]
    fn execution_context_is_validated_and_plain_queries_stay_compatible() {
        let base = serde_json::json!({"requirements":[{"kind":"wand"}]});
        let query = serde_json::json!({"requirements":[{"kind":"wand"},{"kind":"armor"}]});
        let packet = serde_json::json!({"query":query,"refine_base":base}).to_string();
        assert!(decode_execution(packet.as_bytes()).unwrap().1.is_some());
        assert!(
            decode_execution(base.to_string().as_bytes())
                .unwrap()
                .1
                .is_none()
        );
        let unrelated =
            serde_json::json!({"query":{"requirements":[{"kind":"armor"}]},"refine_base":base});
        assert!(decode_execution(unrelated.to_string().as_bytes()).is_err());
        assert!(decode_execution(br#"{"query":{},"refine_base":{},"unknown":true}"#).is_err());
    }
}
