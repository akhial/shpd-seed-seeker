//! One JSON request/response contract shared by C, JNI, wasm and Rust callers.

use super::{
    LevelMap, MapError, SCHEMA_VERSION, assets, generate_level_map_in_branch, validate_location,
};
use crate::catalog::{item, item_by_stable_id};
use crate::challenges::Challenges;
use crate::json_query::{self, CHALLENGE_NAMES};
use crate::seed::DungeonSeed;
use crate::trinkets::{resolve_selection, selection_slots, trinket_order};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    seed: String,
    depth: u8,
    #[serde(default)]
    branch: u8,
    #[serde(default)]
    challenges: Vec<String>,
    #[serde(default)]
    query: Option<Value>,
    /// Absent/null resolves from the query; "none" explicitly deselects.
    #[serde(default)]
    trinket: Option<String>,
}

/// A validated request; query-based selection has already been resolved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelMapRequest {
    pub seed: DungeonSeed,
    pub depth: u8,
    pub branch: u8,
    pub challenges: Challenges,
    pub selected_trinket: Option<crate::catalog::ItemId>,
}

/// Parses a complete seed code, a supported regular depth, challenge names,
/// and optional scout query/initial-offer override. Partial seeds are rejected.
///
/// # Errors
/// Reports malformed JSON, unsupported depths, unknown challenges, invalid
/// queries, and selections outside the seed's four initial offers.
pub fn decode_request(input: &str) -> Result<LevelMapRequest, String> {
    let request: Request = serde_json::from_str(input)
        .map_err(|error| format!("invalid level map request: {error}"))?;
    let seed = DungeonSeed::from_code(&request.seed).map_err(|error| error.to_string())?;
    validate_location(request.depth, request.branch).map_err(|error| error.to_string())?;
    let mut challenges = Challenges::NONE;
    for name in request.challenges {
        challenges |= CHALLENGE_NAMES
            .iter()
            .find(|(known, _)| *known == name)
            .ok_or_else(|| format!("unknown challenge: {name}"))?
            .1;
    }
    let query = request
        .query
        .map(|query| json_query::decode(&query.to_string()))
        .transpose()?;
    let selected_trinket = match request.trinket.as_deref() {
        None => query
            .as_ref()
            .and_then(|q| resolve_selection(seed, &selection_slots(q))),
        Some("none") => None,
        Some(id) => Some(
            item_by_stable_id(id)
                .ok_or_else(|| format!("unknown trinket: {id}"))?
                .id,
        ),
    };
    if selected_trinket.is_some_and(|id| !trinket_order(seed)[..4].contains(&id)) {
        return Err(MapError::InvalidTrinket.to_string());
    }
    Ok(LevelMapRequest {
        seed,
        depth: request.depth,
        branch: request.branch,
        challenges,
        selected_trinket,
    })
}

impl LevelMapRequest {
    /// Generates and serializes the map. Native bridges contain generation
    /// panics in the session layer, like the other scout entry points.
    ///
    /// # Errors
    /// Reports a failed map generation.
    pub fn generate_document(&self) -> Result<String, MapError> {
        let map = generate_level_map_in_branch(
            self.seed,
            self.depth,
            self.branch,
            self.challenges,
            self.selected_trinket,
        )?;
        Ok(document(&map).to_string())
    }
}

/// Serializes a map without regenerating it or reading any runtime state.
#[must_use]
pub fn document(map: &LevelMap) -> Value {
    use crate::level::Feeling;
    let feeling = match map.feeling {
        Feeling::None => "none",
        Feeling::Chasm => "chasm",
        Feeling::Water => "water",
        Feeling::Grass => "grass",
        Feeling::Dark => "dark",
        Feeling::Large => "large",
        Feeling::Traps => "traps",
        Feeling::Secrets => "secrets",
    };

    json!({
        "format": "seed-seeker-level-map",
        "schemaVersion": SCHEMA_VERSION,
        "shpdVersion": crate::SHPD_VERSION,
        "shpdCommit": crate::SHPD_COMMIT,
        "profile": "canonical-scout-raised-v2",
        "seed": map.seed.to_code(),
        "depth": map.depth,
        "branch": map.kind.branch(),
        "kind": map.kind,
        "branches": map.branches,
        "challenges": CHALLENGE_NAMES.iter().filter(|(_, flag)| map.challenges.contains(*flag)).map(|(name, _)| name).collect::<Vec<_>>(),
        "selectedTrinket": map.selected_trinket.map(|id| item(id).stable_id),
        "feeling": feeling,
        "width": map.width,
        "height": map.height,
        "terrain": map.terrain,
        "entrance": map.entrance,
        "exit": map.exit,
        "secretRooms": map.secret_rooms,
        "secretDoors": map.terrain.iter().enumerate().filter_map(|(cell, &tile)| (tile == crate::geometry::terrain::SECRET_DOOR).then_some(cell)).collect::<Vec<_>>(),
        "secretTraps": map.terrain.iter().enumerate().filter_map(|(cell, &tile)| (tile == crate::geometry::terrain::SECRET_TRAP).then_some(cell)).collect::<Vec<_>>(),
        "traps": map.traps,
        "assetRevision": assets::SOURCE_REVISION,
        "assets": assets::ASSETS.iter().filter(|asset| map.scene.sprites.iter().flat_map(|sprite| &sprite.frames).flatten().any(|draw| matches!(draw, super::MapDraw::Blit {asset: id,..} if *id == asset.id))).collect::<Vec<_>>(),
        "scene": map.scene,
    })
}
