//! Deterministic seed-search primitives for Shattered Pixel Dungeon v3.3.x.
//!
//! The compatibility boundary is intentionally explicit: all random generation
//! must flow through [`rng`] so Java parity can be tested independently from
//! higher-level dungeon generation.

pub mod auto_trinkets;
pub mod batch;
mod bit_rows;
pub mod builder;
pub mod catalog;
pub mod caves_floor;
pub mod caves_mobs;
pub mod caves_rooms;
pub mod challenges;
pub mod city_boss_shop;
pub mod city_floor;
pub mod city_mobs;
pub mod city_rooms;
pub mod deep_link;
#[cfg(feature = "json-query")]
pub mod engine_info;
pub mod equipment;
pub mod feasibility;
pub mod generator;
pub mod geometry;
pub mod grid_builder;
pub mod halls_floor;
pub mod halls_mobs;
pub mod halls_rooms;
pub mod java_math;
#[cfg(feature = "json-query")]
pub mod json_query;
pub mod level;
pub mod level_flags;
pub mod level_map;
pub mod level_prelude;
pub mod main_world;
mod maze;
pub mod mining_floor;
pub mod mobs;
pub mod model;
pub mod painter;
pub mod prison_floor;
pub mod prison_mobs;
pub mod prison_rooms;
pub mod probability;
pub mod probability_tables;
pub mod query;
pub mod quest_rooms;
pub mod quests;
pub mod regular_items;
pub mod regular_level;
pub mod regular_placement;
#[cfg(feature = "json-query")]
pub mod results_export;
pub mod rng;
pub mod room;
pub mod room_decks;
pub mod run;
pub mod search;
pub mod secret_rooms;
pub mod seed;
pub mod sewer_floor;
pub mod sewer_mob_placement;
pub mod sewer_rooms;
pub mod shop;
pub mod special_consumable;
pub mod special_equipment;
pub mod special_forced;
pub mod trinkets;
#[cfg(test)]
mod vault_debug;
pub mod vault_floor;
pub mod vault_loot;
pub mod vault_mobs;
pub mod vault_paint;
pub mod vault_rooms;
pub mod wire;

/// Upstream generation line this engine targets.
pub const SHPD_VERSION: &str = "4.0.0";

/// Upstream source commit tagged `v4.0.0`. The matching official release
/// JAR and its SHA-256 digest are pinned separately in `tooling/oracle-4.0`.
pub const SHPD_COMMIT: &str = "2bb34a4e91d29c8785a9363cad6ddfe5122b1d4f";
