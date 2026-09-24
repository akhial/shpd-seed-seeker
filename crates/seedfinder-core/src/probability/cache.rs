//! Reuse pure supply calculations across query analysis and trinket scoring.
//!
//! Predicates include the profile, depth, source and every item filter. The
//! tables are immutable, so these answers do not depend on the active query
//! or search results. Intermediate supply calculations stay thread-local;
//! complete query/profile estimates survive switching frontend worker threads.
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use super::{Predicate, Profile};
use crate::query::SearchQuery;

// Enough for the final estimate and five equipment profiles of eight recent
// queries. Full equality includes resin, floor rules, challenges and all filters.
const QUERY_LIMIT: usize = 64;
type QueryEntry = (SearchQuery, Option<Profile>, Arc<OnceLock<f64>>);
static QUERIES: QueryCache = QueryCache(Mutex::new(VecDeque::new()));

struct QueryCache(Mutex<VecDeque<QueryEntry>>);

impl QueryCache {
    fn get_or_compute(
        &self,
        query: &SearchQuery,
        profile: Option<Profile>,
        compute: impl FnOnce() -> f64,
    ) -> f64 {
        let value = {
            let mut entries = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let entry = entries
                .iter()
                .position(|(cached, effect, _)| cached == query && *effect == profile)
                .and_then(|index| entries.remove(index))
                .unwrap_or_else(|| (query.clone(), profile, Arc::new(OnceLock::new())));
            let value = Arc::clone(&entry.2);
            entries.push_back(entry);
            if entries.len() > QUERY_LIMIT {
                entries.pop_front();
            }
            value
        };
        // Coalesce callers for this key without holding the cache lock during
        // calculation. A final estimate can request cached equipment profiles.
        *value.get_or_init(compute)
    }
}

pub(super) fn query(
    query: &SearchQuery,
    profile: Option<Profile>,
    compute: impl FnOnce() -> f64,
) -> f64 {
    QUERIES.get_or_compute(query, profile, compute)
}

type ResinKey = (
    Vec<Predicate>,
    Vec<Predicate>,
    Predicate,
    bool,
    u16,
    u8,
    bool,
);

const LIMIT: usize = 4096;

thread_local! {
    static MEANS: RefCell<HashMap<Predicate, f64>> = RefCell::default();
    static OPEN: RefCell<HashMap<Vec<Predicate>, f64>> = RefCell::default();
    static MATCHING: RefCell<HashMap<Vec<Predicate>, f64>> = RefCell::default();
    static RESIN: RefCell<HashMap<ResinKey, f64>> = RefCell::default();
}

pub(super) fn resin(
    predicates: &[Predicate],
    witnesses: &[Predicate],
    donor: Predicate,
    query: &SearchQuery,
    compute: impl FnOnce() -> f64,
) -> f64 {
    let key = (
        predicates.to_vec(),
        witnesses.to_vec(),
        donor,
        query.arcane_resin_auto,
        if query.arcane_resin_auto {
            0
        } else {
            query.arcane_resin
        },
        query.max_depth,
        query.arcane_resin_filter.include_mage_wand,
    );
    RESIN.with(|cache| {
        if let Some(value) = cache.borrow().get(&key) {
            return *value;
        }
        let value = compute();
        let mut cache = cache.borrow_mut();
        if cache.len() >= LIMIT {
            cache.clear();
        }
        cache.insert(key, value);
        value
    })
}

pub(super) fn mean(predicate: Predicate, compute: impl FnOnce() -> f64) -> f64 {
    MEANS.with(|cache| {
        if let Some(value) = cache.borrow().get(&predicate) {
            return *value;
        }
        let value = compute();
        let mut cache = cache.borrow_mut();
        if cache.len() >= LIMIT {
            cache.clear();
        }
        cache.insert(predicate, value);
        value
    })
}

fn lookup(
    cache: &RefCell<HashMap<Vec<Predicate>, f64>>,
    predicates: &[Predicate],
    compute: impl FnOnce() -> f64,
) -> f64 {
    if let Some(value) = cache.borrow().get(predicates) {
        return *value;
    }
    let value = compute();
    let mut cache = cache.borrow_mut();
    if cache.len() >= LIMIT {
        cache.clear();
    }
    cache.insert(predicates.to_vec(), value);
    value
}

pub(super) fn open(predicates: &[Predicate], compute: impl FnOnce() -> f64) -> f64 {
    OPEN.with(|cache| lookup(cache, predicates, compute))
}

pub(super) fn matching(predicates: &[Predicate], compute: impl FnOnce() -> f64) -> f64 {
    MATCHING.with(|cache| lookup(cache, predicates, compute))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Barrier,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn concurrent_workers_share_complete_estimates_and_profiles() {
        let cache = QueryCache(Mutex::new(VecDeque::new()));
        let query = crate::json_query::decode(r#"{"requirements":[{"kind":"wand"}]}"#).unwrap();
        let calls = AtomicUsize::new(0);
        let barrier = Barrier::new(4);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    barrier.wait();
                    let value = cache.get_or_compute(&query, None, || {
                        // Nested profile lookup must not deadlock the shared cache.
                        cache.get_or_compute(&query, Some(Profile::None), || {
                            calls.fetch_add(1, Ordering::Relaxed);
                            0.25
                        })
                    });
                    assert_eq!(value.to_bits(), 0.25_f64.to_bits());
                });
            }
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            cache
                .get_or_compute(&query, Some(Profile::None), || panic!("profile recomputed"))
                .to_bits(),
            0.25_f64.to_bits()
        );
        assert_eq!(
            cache
                .get_or_compute(&query, Some(Profile::MimicTooth), || 0.5)
                .to_bits(),
            0.5_f64.to_bits()
        );
    }

    #[test]
    fn edits_invalidate_estimates_and_storage_is_bounded_including_unknowns() {
        let cache = QueryCache(Mutex::new(VecDeque::new()));
        let query = crate::json_query::decode(r#"{"requirements":[{"kind":"wand"}]}"#).unwrap();
        assert!(cache.get_or_compute(&query, None, || f64::NAN).is_nan());
        assert!(
            cache
                .get_or_compute(&query, None, || panic!("unknown recomputed"))
                .is_nan()
        );
        for amount in 1..=u16::try_from(QUERY_LIMIT).unwrap() {
            let edited = SearchQuery {
                arcane_resin: amount,
                ..query.clone()
            };
            assert_eq!(
                cache
                    .get_or_compute(&edited, None, || f64::from(amount))
                    .to_bits(),
                f64::from(amount).to_bits()
            );
        }
        assert_eq!(cache.0.lock().unwrap().len(), QUERY_LIMIT);
        let mut recalculated = false;
        cache.get_or_compute(&query, None, || {
            recalculated = true;
            f64::NAN
        });
        assert!(recalculated, "oldest estimate must have been evicted");
    }

    #[test]
    fn android_anr_query_keeps_its_estimate_across_planning_and_worker_changes() {
        // Reported Galaxy S10 query: automatic resin/trinkets, repeated rings,
        // artifacts and an early-floor wand blanket. No worlds need generation.
        let query = crate::deep_link::decode_text(
            "https://shpd-seed-seeker.web.app/#q=q6gAAAuW4ABLYAAlwAAXPGABc8AZhc8AZh-sAA_cAANuQKAdkLACCIIx",
        ).unwrap();
        let plan = std::thread::scope(|scope| {
            scope
                .spawn(|| crate::feasibility::QueryPlan::analyze(&query))
                .join()
                .unwrap()
        });
        assert!(!plan.is_unsatisfiable());
        for _ in 0..2 {
            let actual = std::thread::scope(|scope| {
                scope
                    .spawn(|| crate::probability::estimate_match_probability(&query))
                    .join()
                    .unwrap()
            });
            // Captured from the uncached estimator before this fix.
            let expected = 1.508_046_643_725_133_8e-8;
            assert!((actual / expected - 1.0).abs() < 1e-10, "{actual}");
        }
    }
}
