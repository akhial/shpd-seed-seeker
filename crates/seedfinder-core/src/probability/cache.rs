//! Reuse pure supply calculations across query analysis and trinket scoring.
//!
//! Predicates include the profile, depth, source and every item filter. The
//! tables are immutable, so these answers do not depend on the active query
//! or search results. Bound storage and keep it local to each worker thread.
use std::{cell::RefCell, collections::HashMap};

use super::Predicate;
use crate::query::SearchQuery;

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
