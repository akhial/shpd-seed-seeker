//! Joint supply and resin-balance estimate. Each item is spent once: on the
//! scarcest outstanding reservation or donor witness it covers, or as surplus
//! resin. Donor witnesses contribute yield without adding upgrade costs.
//! Upgrade outcomes are integrated in one pass, including mixed upgrades.
//! As in the equipment model, supply counts use the measured variance and
//! quest offers form one mutually exclusive choice, never independent donors.
use std::collections::BTreeMap;

use super::{
    Coverages, HIGHEST_TABLED_UPGRADE, Predicate, Profile, STATE_FLOOR, STATE_LIMIT, Stream,
    binomial_counts, effective_requirements, equipment_probability, expected_slots, filters,
    poisson_counts, repeated_identity,
};
use crate::{
    catalog::ItemKind,
    probability_tables::{
        DEPTHS, LINES_ORDER, Line, PRIZE_GROUPS, Supply, prize_group, source_index, spread_index,
    },
    query::{SearchQuery, resin_donor_requirement as donor_requirement, resin_upgrade_cost},
};

pub(super) fn probability(query: &SearchQuery, profile: Profile) -> f64 {
    let mut ordinary = query.clone();
    ordinary.arcane_resin = 0;
    ordinary.arcane_resin_auto = false;
    let donor = donor_requirement(query);
    // Every eligible donor yields at least two resin, so this is exactly one
    // extra ordinary wand and should use the same equipment calculation.
    if !query.arcane_resin_auto && query.arcane_resin <= 2 {
        ordinary.requirements.push(donor);
        return equipment_probability(&ordinary, profile);
    }
    let baseline = equipment_probability(&ordinary, profile);
    let ordered = filters(
        query,
        &effective_requirements(query, profile),
        None,
        &[],
        profile,
    );
    with_resin(query, profile, &ordered, &[], baseline)
}

/// Apply resin to an ordinary assignment, after any blanket intersections.
/// Witness donors occupy their own supply slots, contribute resin once and
/// add no Auto cost. Ordinary reservations cannot donate their resin.
pub(super) fn with_resin(
    query: &SearchQuery,
    profile: Profile,
    ordered: &[Predicate],
    witnesses: &[Predicate],
    baseline: f64,
) -> f64 {
    if !baseline.is_finite() {
        return baseline;
    }
    if baseline <= STATE_FLOOR {
        return 0.0;
    }
    if query.arcane_resin_auto
        && ordered
            .iter()
            .all(|p| p.kind != ItemKind::Wand || p.upgrades.trailing_zeros() >= 3)
    {
        return if witnesses.is_empty() { baseline } else { 0.0 };
    }
    let donor = donor_requirement(query);
    let donor = Predicate::of(donor, None)
        .within(query, &donor)
        .with_profile(profile);
    if !query.arcane_resin_auto
        && count_bound(
            expected_slots(&donor),
            query
                .arcane_resin
                .div_ceil(2 * (u16::from(HIGHEST_TABLED_UPGRADE) + 1)),
        ) <= STATE_FLOOR
    {
        return 0.0;
    }
    if !query.arcane_resin_auto && query.arcane_resin <= 2 {
        if !witnesses.is_empty() {
            return baseline;
        }
        let mut allocated = ordered.to_vec();
        allocated.push(donor);
        super::sort_filters(&mut allocated);
        return baseline
            * (super::matching_chance(&allocated) / super::matching_chance(ordered))
                .clamp(0.0, 1.0);
    }
    baseline
        * super::cache::resin(ordered, witnesses, donor, query, || {
            conditional_probability(query, profile, ordered, witnesses, donor)
        })
}

fn conditional_probability(
    query: &SearchQuery,
    profile: Profile,
    ordered: &[Predicate],
    witnesses: &[Predicate],
    donor: Predicate,
) -> f64 {
    let mut allocated: Vec<_> = ordered
        .iter()
        .map(|p| (*p, false))
        .chain(witnesses.iter().map(|p| (*p, true)))
        .collect();
    allocated
        .sort_by(|(left, _), (right, _)| expected_slots(left).total_cmp(&expected_slots(right)));
    let mut predicates = Vec::new();
    let mut donors = Vec::new();
    let mut wanted = Vec::new();
    for &(p, is_donor) in &allocated {
        if let Some(index) = predicates
            .iter()
            .zip(&donors)
            .position(|(held, donor)| *held == p && *donor == is_donor)
        {
            wanted[index] += 1;
        } else {
            predicates.push(p);
            donors.push(is_donor);
            wanted.push(1);
        }
    }
    let mut start = 0;
    let slots = wanted
        .into_iter()
        .map(|count| {
            let range = start..start + count;
            start += count;
            range
        })
        .collect();
    let model = Model {
        auto: query.arcane_resin_auto,
        slots,
        wands: predicates
            .iter()
            .zip(&donors)
            .map(|(p, is_donor)| !is_donor && p.kind == ItemKind::Wand)
            .collect(),
        donors,
    };
    predicates.push(donor);
    let shared = Coverages::of(&predicates);
    let covers: Vec<_> = (0..shared.len())
        .map(|coverage| shared.members(coverage))
        .collect();
    let mut states = BTreeMap::from([(
        State {
            held: vec![EMPTY; allocated.len()].into_boxed_slice(),
            balance: if model.auto {
                0
            } else {
                -i32::from(query.arcane_resin)
            },
        },
        1.0,
    )]);
    let supply = SupplyPlan {
        predicates,
        shared,
        covers,
    };
    states = supply.prizes(&model, profile, query.max_depth, states);
    let allocated: Vec<_> = allocated.into_iter().map(|(p, _)| p).collect();
    states = supply.open(&model, profile, &allocated, states);
    let mut ordinary_mass = 0.0;
    let mut resin_mass = 0.0;
    for (state, mass) in states {
        if Model::complete(&state) {
            ordinary_mass += mass;
            if state.balance >= 0 {
                resin_mass += mass;
            }
        }
    }
    // Retain the equipment model's identity-deck correction and world filters;
    // use the joint model for the conditional chance of sufficient surplus.
    if ordinary_mass > 0.0 {
        (resin_mass / ordinary_mass).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

struct SupplyPlan {
    predicates: Vec<Predicate>,
    shared: Coverages,
    covers: Vec<Vec<usize>>,
}

type WeightedOffers = Vec<(f64, Offer)>;
type Bundles = BTreeMap<(usize, usize), (u8, WeightedOffers)>;

impl SupplyPlan {
    fn prizes(&self, model: &Model, profile: Profile, max_depth: u8, mut states: States) -> States {
        // Resolve reward choices together across families before the open supply.
        for group in PRIZE_GROUPS {
            let mut choices = Vec::new();
            for depth in 1..=usize::from(max_depth) {
                let mut appeared = 0.0_f64;
                let mut offers = Vec::new();
                for kind in kinds(&self.predicates) {
                    for supply in profile
                        .supply_for(kind)
                        .filter(|s| prize_group(s.source) == Some(group))
                    {
                        appeared = appeared.max(f64::from(supply.depth_slots[depth - 1]));
                        if supply.depth_slots[depth - 1] > 0.0 {
                            offers.push(offer(
                                &self.shared,
                                &self.covers,
                                &supply,
                                depth,
                                self.predicates.len() - 1,
                            ));
                        }
                    }
                }
                if appeared > 0.0 {
                    choices.push((appeared, offers));
                }
            }
            if !choices.is_empty() {
                states = model.prize(states, &choices);
            }
        }
        states
    }
    fn open(
        &self,
        model: &Model,
        profile: Profile,
        ordered: &[Predicate],
        mut states: States,
    ) -> States {
        let mut limits: Vec<_> = self
            .predicates
            .iter()
            .map(|p| usize::from(p.max_depth).min(DEPTHS))
            .collect();
        limits.sort_unstable();
        limits.dedup();
        for kind in kinds(&self.predicates) {
            let steady = repeated_identity(
                &ordered
                    .iter()
                    .copied()
                    .filter(|p| p.kind == kind)
                    .collect::<Vec<_>>(),
            )
            .is_none();
            for line in LINES_ORDER
                .into_iter()
                .filter(|line| kind == ItemKind::Weapon || *line == Line::Plain)
            {
                for (from, until) in super::stretches(&limits) {
                    let mut placed = 0.0;
                    let mut offers = Vec::new();
                    let mut bundles: Bundles = BTreeMap::new();
                    for supply in profile
                        .supply_for(kind)
                        .filter(|s| s.line == line && prize_group(s.source).is_none())
                    {
                        for depth in from..=until {
                            let available = f64::from(supply.depth_slots[depth - 1]);
                            if available <= 0.0 {
                                continue;
                            }
                            let offer = offer(
                                &self.shared,
                                &self.covers,
                                &supply,
                                depth,
                                self.predicates.len() - 1,
                            );
                            if supply.bundle == 0 {
                                placed += available;
                                offers.push((available, offer));
                            } else {
                                bundles
                                    .entry((source_index(supply.source), depth))
                                    .or_insert((supply.bundle, Vec::new()))
                                    .1
                                    .push((available / f64::from(supply.bundle), offer));
                            }
                        }
                    }
                    if placed > 0.0 {
                        let stream = Stream::of(
                            profile,
                            spread_index(kind, line),
                            until,
                            placed,
                            Vec::new(),
                            steady,
                        );
                        let counts = supply_counts(&stream, placed);
                        for (weight, _) in &mut offers {
                            *weight /= placed;
                        }
                        let offers = collapse(offers);
                        states = model.stream(states, &offers, &counts);
                    }
                    for (count, mut offers) in bundles.into_values() {
                        let total: f64 = offers.iter().map(|(weight, _)| weight).sum();
                        for (weight, _) in &mut offers {
                            *weight /= total.max(1.0);
                        }
                        let offers = collapse(offers);
                        for _ in 0..count {
                            states = model.draw(states, &offers);
                        }
                    }
                    if kind != ItemKind::Weapon {
                        states = self.condition_on_closed_slots(model, kind, until, states);
                    }
                }
            }
            states = self.condition_on_closed_slots(model, kind, usize::MAX, states);
        }
        states
    }

    fn condition_on_closed_slots(
        &self,
        model: &Model,
        kind: ItemKind,
        depth: usize,
        mut states: States,
    ) -> States {
        // All prizes and this family's open supply through `depth` are done.
        // States missing a slot whose floor limit has closed can contribute to
        // neither side of the conditional ratio. Rescale the surviving mass to
        // preserve rare combinations and avoid carrying failed early donors
        // through the entire dungeon. Weapons close after all generator lines.
        states.retain(|state, _| {
            model.slots.iter().enumerate().all(|(index, range)| {
                self.predicates[index].kind != kind
                    || usize::from(self.predicates[index].max_depth) > depth
                    || state.held[range.end - 1] != EMPTY
            })
        });
        let mass: f64 = states.values().sum();
        if mass > 0.0 {
            for probability in states.values_mut() {
                *probability /= mass;
            }
        }
        states
    }
}

/// Include the whole bounded run, or a Poisson tail below numerical precision.
/// Counting only as many draws as the resin needs would drop later uncursed
/// donors whenever an earlier draw was cursed or reserved for a requirement.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Nonnegative, small measured supply counts.
fn supply_counts(stream: &Stream, mean: f64) -> Vec<f64> {
    stream.trials.map_or_else(
        || poisson_counts(mean, (mean + 12.0 * mean.sqrt() + 32.0).ceil() as usize),
        |trials| binomial_counts(trials, (mean / trials).min(1.0), trials.ceil() as usize),
    )
}

fn kinds(predicates: &[Predicate]) -> Vec<ItemKind> {
    let mut kinds = Vec::new();
    for predicate in predicates {
        if !kinds.contains(&predicate.kind) {
            kinds.push(predicate.kind);
        }
    }
    kinds
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Event {
    /// Distinct reservation or donor predicates, scarcest first.
    covers: Vec<usize>,
    /// Yield if consumed, zero when it fails the donor filters.
    resin: i32,
    /// Cost to bring this generated wand to +3 (zero for other kinds).
    cost: i32,
}
struct Offer {
    outcomes: Vec<(Event, f64)>,
    options: f64,
}

fn collapse(offers: Vec<(f64, Offer)>) -> Vec<(f64, Offer)> {
    let mut single = BTreeMap::new();
    let mut mass = 0.0;
    let mut choices = Vec::new();
    for (weight, offer) in offers {
        if (offer.options - 1.0).abs() < f64::EPSILON {
            mass += weight;
            for (event, chance) in offer.outcomes {
                *single.entry(event).or_insert(0.0) += weight * chance;
            }
        } else {
            choices.push((weight, offer));
        }
    }
    if mass > 0.0 {
        choices.push((
            mass,
            Offer {
                outcomes: single
                    .into_iter()
                    .map(|(event, chance)| (event, chance / mass))
                    .collect(),
                options: 1.0,
            },
        ));
    }
    choices
}

fn offer(
    shared: &Coverages,
    covers: &[Vec<usize>],
    supply: &Supply,
    depth: usize,
    donor: usize,
) -> Offer {
    let mut outcomes = BTreeMap::new();
    let mut single = *supply;
    single.options = 1.0;
    let levels: Vec<_> = if supply.kind == ItemKind::Wand {
        supply
            .upgrades
            .iter()
            .enumerate()
            .filter(|(_, chance)| **chance > 0.0)
            .map(|(level, chance)| (level, f64::from(*chance)))
            .collect()
    } else {
        vec![(0, 1.0)]
    };
    for (level, chance) in levels {
        if supply.kind == ItemKind::Wand {
            single.upgrades.fill(0.0);
            single.upgrades[level] = 1.0;
        }
        for (coverage, share) in shared
            .shares(&single, depth)
            .into_iter()
            .enumerate()
            .skip(1)
        {
            if share <= 0.0 {
                continue;
            }
            let event = Event {
                covers: covers[coverage]
                    .iter()
                    .copied()
                    .filter(|&member| member != donor)
                    .collect(),
                resin: if covers[coverage].contains(&donor) {
                    2 * i32::try_from(level + 1).unwrap()
                } else {
                    0
                },
                cost: if supply.kind == ItemKind::Wand {
                    i32::try_from(resin_upgrade_cost(u8::try_from(level).unwrap())).unwrap()
                } else {
                    0
                },
            };
            *outcomes.entry(event).or_insert(0.0) += chance * share;
        }
    }
    let total: f64 = outcomes.values().sum();
    Offer {
        outcomes: outcomes
            .into_iter()
            .map(|(event, mass)| (event, mass / total.max(1.0)))
            .collect(),
        options: f64::from(supply.options),
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct State {
    /// Sorted opportunity costs in each predicate's fixed range: donor yield
    /// plus Auto upgrade cost for reservations, zero for consumed witnesses,
    /// or EMPTY for an outstanding requirement.
    held: Box<[u8]>,
    /// Surplus resin less the selected wands' upgrade costs (Auto) or the
    /// fixed target. Nonnegative means the resin condition is satisfied.
    balance: i32,
}
struct Model {
    auto: bool,
    slots: Vec<std::ops::Range<usize>>,
    wands: Vec<bool>,
    /// Slots whose selected item is consumed as resin and witnesses blankets.
    donors: Vec<bool>,
}
type States = BTreeMap<State, f64>;
const EMPTY: u8 = u8::MAX;

impl Model {
    fn target(&self, state: &State, event: &Event) -> Option<usize> {
        event
            .covers
            .iter()
            .copied()
            .find(|&index| state.held[self.slots[index].end - 1] == EMPTY)
    }
    fn rank(&self, state: &State, event: &Event) -> (usize, i32) {
        match self.target(state, event) {
            Some(index) if self.donors[index] => (index, -event.resin),
            Some(index) => (index, if self.auto { event.cost } else { event.resin }),
            None => (self.slots.len(), -event.resin),
        }
    }
    fn complete(state: &State) -> bool {
        !state.held.contains(&EMPTY)
    }
    fn take(&self, state: &State, event: &Event) -> State {
        if Self::complete(state) && state.balance >= 0 {
            return state.clone();
        }
        let mut next = state.clone();
        let cost = event.resin + if self.auto { event.cost } else { 0 };
        let stored_cost = u8::try_from(cost).unwrap();
        next.balance += event.resin;
        if let Some(index) = self.target(state, event) {
            if self.donors[index] {
                insert_cost(&mut next.held[self.slots[index].clone()], 0);
            } else {
                insert_cost(&mut next.held[self.slots[index].clone()], stored_cost);
                next.balance -= cost;
            }
        } else if let Some((index, previous)) = event
            .covers
            .iter()
            .filter_map(|&index| {
                let previous = i32::from(state.held[self.slots[index].end - 1]);
                (previous > cost).then_some((index, previous))
            })
            .max_by_key(|&(_, previous)| previous)
        {
            // A later, cheaper matching wand replaces an earlier reservation.
            // Its predecessor becomes a donor. This integrates all upgrade
            // mixtures instead of forcing every reserved wand to one floor.
            insert_cost(&mut next.held[self.slots[index].clone()], stored_cost);
            next.balance += previous - cost;
        }
        let remaining = if self.auto {
            self.slots
                .iter()
                .zip(&self.wands)
                .filter(|(_, wand)| **wand)
                .map(|(range, _)| {
                    (range.len() - next.held[range.clone()].partition_point(|&cost| cost != EMPTY))
                        * 6
                })
                .sum()
        } else {
            0
        };
        next.balance = next
            .balance
            .min(i32::try_from(remaining).unwrap_or(i32::MAX));
        if Self::complete(&next) && next.balance >= 0 {
            // Once satisfied, reservation costs cannot change success.
            next.held.fill(0);
        }
        next
    }
    fn choices(&self, state: &State, offers: &[Offer]) -> Vec<(State, f64)> {
        if let [offer] = offers
            && (offer.options - 1.0).abs() < f64::EPSILON
        {
            let mut outcomes: Vec<_> = offer
                .outcomes
                .iter()
                .map(|(event, chance)| (self.take(state, event), *chance))
                .collect();
            let mass: f64 = offer.outcomes.iter().map(|(_, chance)| chance).sum();
            outcomes.push((state.clone(), (1.0 - mass).max(0.0)));
            return outcomes;
        }
        let mut events: Vec<_> = offers
            .iter()
            .flat_map(|offer| offer.outcomes.iter().map(|(event, _)| event))
            .collect();
        events.sort_by_key(|event| (self.rank(state, event), *event));
        events.dedup();
        let mut cumulative = vec![0.0; offers.len()];
        let mut missing = 1.0;
        let mut choices = Vec::new();
        for event in events {
            for (index, offer) in offers.iter().enumerate() {
                cumulative[index] += offer
                    .outcomes
                    .iter()
                    .filter(|(other, _)| other == event)
                    .map(|(_, chance)| chance)
                    .sum::<f64>();
            }
            let next_missing: f64 = offers
                .iter()
                .zip(&cumulative)
                .map(|(offer, &mass)| (1.0 - mass).clamp(0.0, 1.0).powf(offer.options))
                .product();
            if missing > next_missing {
                choices.push((self.take(state, event), missing - next_missing));
            }
            missing = next_missing;
        }
        choices.push((state.clone(), missing));
        choices
    }
    fn draw(&self, states: States, offers: &[(f64, Offer)]) -> States {
        let mut next = BTreeMap::new();
        let mut missed = (1.0 - offers.iter().map(|(weight, _)| weight).sum::<f64>()).max(0.0);
        for (weight, offer) in offers {
            if (offer.options - 1.0).abs() < f64::EPSILON {
                let mass: f64 = offer.outcomes.iter().map(|(_, chance)| chance).sum();
                missed += weight * (1.0 - mass).max(0.0);
            }
        }
        for (state, mass) in states {
            for (weight, offer) in offers {
                if (offer.options - 1.0).abs() < f64::EPSILON {
                    for (event, chance) in &offer.outcomes {
                        *next.entry(self.take(&state, event)).or_insert(0.0) +=
                            mass * weight * chance;
                    }
                } else {
                    for (outcome, chance) in self.choices(&state, std::slice::from_ref(offer)) {
                        *next.entry(outcome).or_insert(0.0) += mass * weight * chance;
                    }
                }
            }
            *next.entry(state).or_insert(0.0) += mass * missed;
        }
        prune(next)
    }
    fn prize(&self, states: States, choices: &[(f64, Vec<Offer>)]) -> States {
        let mut next = BTreeMap::new();
        let missed = (1.0 - choices.iter().map(|(weight, _)| weight).sum::<f64>()).max(0.0);
        for (state, mass) in states {
            for (weight, offers) in choices {
                for (outcome, chance) in self.choices(&state, offers) {
                    *next.entry(outcome).or_insert(0.0) += mass * weight * chance;
                }
            }
            *next.entry(state).or_insert(0.0) += mass * missed;
        }
        prune(next)
    }
    fn stream(&self, mut states: States, offers: &[(f64, Offer)], counts: &[f64]) -> States {
        let mut mixed = BTreeMap::new();
        for (count, &chance) in counts.iter().enumerate() {
            if count > 0 {
                states = self.draw(states, offers);
            }
            if chance > 0.0 {
                for (state, mass) in &states {
                    *mixed.entry(state.clone()).or_insert(0.0) += mass * chance;
                }
            }
        }
        prune(mixed)
    }
}

/// Fill an empty reservation or replace the most expensive one. Fixed ranges
/// keep the state in one compact allocation, independent of duplicate count.
fn insert_cost(held: &mut [u8], cost: u8) {
    let end = held
        .partition_point(|&held| held != EMPTY)
        .min(held.len() - 1);
    let position = held[..end].partition_point(|&held| held <= cost);
    held.copy_within(position..end, position + 1);
    held[position] = cost;
}

fn prune(mut states: States) -> States {
    states.retain(|_, mass| *mass > STATE_FLOOR);
    if states.len() > STATE_LIMIT {
        let mut masses: Vec<_> = states.values().copied().collect();
        masses.select_nth_unstable_by(STATE_LIMIT, |a, b| b.total_cmp(a));
        states.retain(|_, mass| *mass > masses[STATE_LIMIT]);
    }
    states
}

fn count_bound(mean: f64, count: u16) -> f64 {
    if count == 0 {
        return 1.0;
    }
    if mean <= 0.0 {
        return 0.0;
    }
    let count = f64::from(count);
    if count <= mean {
        1.0
    } else {
        (count * (1.0 + (mean / count).ln()) - mean).exp()
    }
}
