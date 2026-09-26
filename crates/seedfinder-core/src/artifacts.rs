//! Starting and remaining artifact decks. Depth 0 is the full starting deck.
//! Reading never consumes world RNG.
use crate::{
    catalog::{ItemId, ItemKind, item},
    generator::{ARTIFACT_ITEMS, select_artifact_identity_index},
    model::GeneratedWorld,
    rng::RandomStack,
    run::GeneratorState,
};

pub const TRANSMUTATION_COUNT: u8 = 10;

/// A compact snapshot of the private category RNG and remaining identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDeck {
    pub depth: u8,
    pub order: Vec<ItemId>,
}

impl ArtifactDeck {
    pub(crate) fn capture(depth: u8, generator: &GeneratorState) -> Self {
        let mut generator = generator.clone();
        let mut random = RandomStack::with_base_seed(0);
        let mut order = Vec::new();
        while let Some(index) = select_artifact_identity_index(&mut random, &mut generator)
            .expect("initialized artifact deck")
        {
            order.push(ARTIFACT_ITEMS[index].item_id().expect("spawnable artifact"));
        }
        Self { depth, order }
    }
}

/// Boss floors without new artifact generation inherit the preceding deck.
#[must_use]
pub fn deck_at(world: &GeneratedWorld, depth: u8) -> &[ItemId] {
    world
        .artifact_decks
        .iter()
        .rev()
        .find(|deck| deck.depth <= depth)
        .map_or(&[], |deck| deck.order.as_slice())
}

#[derive(Clone, Debug)]
pub(crate) struct Outcome {
    pub donor: usize,
    pub depth: u8,
    pub step: usize,
    pub identity: ItemId,
}

/// Outcomes have the donor's curse, source and accessibility. Different donors
/// cannot duplicate a unique artifact; assignment tracks both donor and identity.
pub(crate) fn outcomes(query: &crate::query::SearchQuery, world: &GeneratedWorld) -> Vec<Outcome> {
    let mut limits = std::collections::BTreeMap::<u8, u8>::new();
    for requirement in &query.requirements {
        if requirement.artifact_transmutations > 0 {
            let depth = requirement
                .max_depth
                .unwrap_or(query.max_depth)
                .min(query.max_depth);
            let limit = limits.entry(depth).or_default();
            *limit = (*limit).max(requirement.artifact_transmutations);
        }
    }
    let mut outcomes = Vec::new();
    for (depth, limit) in limits {
        for (donor, candidate) in world.items.iter().enumerate() {
            if candidate.depth > depth || item(candidate.item).kind != ItemKind::Artifact {
                continue;
            }
            for (step, &identity) in deck_at(world, depth)
                .iter()
                .take(usize::from(limit))
                .enumerate()
            {
                outcomes.push(Outcome {
                    donor,
                    depth,
                    step,
                    identity,
                });
            }
        }
    }
    outcomes
}
