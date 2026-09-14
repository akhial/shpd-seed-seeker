//! ItemSprite.Glowing colours and fade-in periods, matching the Scout item icons.
use crate::catalog::{Effect, ItemKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapGlow {
    pub color: [u8; 3],
    /// Fade in from zero to 60%, then out over the same interval.
    pub period_ms: u16,
}

impl MapGlow {
    pub(super) fn for_item(kind: ItemKind, effect: Option<Effect>, cursed: bool) -> Option<Self> {
        if kind == ItemKind::Wand {
            return None;
        }
        let (color, period_ms) = if let Some(effect) = effect.filter(|e| !e.is_curse()) {
            match effect.wire_name() {
                "Blazing" | "Brimstone" => ([255, 68, 0], 1000),
                "Chilling" => ([0, 255, 255], 1000),
                "Kinetic" | "Swiftness" => ([255, 255, 0], 1000),
                "Shocking" => ([255, 255, 255], 500),
                "Venomous" => ([68, 0, 170], 1000),
                "Blocking" | "Flow" => ([0, 0, 255], 1000),
                "Blooming" => ([0, 136, 0], 1000),
                "Eldritch" | "Stone" => ([34, 34, 34], 1000),
                "Elastic" => ([255, 0, 255], 1000),
                "Lucky" => ([0, 255, 0], 1000),
                "Projecting" | "Viscosity" => ([136, 68, 204], 1000),
                "Unstable" => ([153, 153, 153], 1000),
                "Vorpal" => ([170, 102, 102], 1000),
                "Corrupting" => ([68, 0, 102], 1000),
                "Crystal" => ([0, 136, 255], 1000),
                "Grim" => ([0, 0, 0], 1000),
                "Vampiric" | "Thorns" => ([102, 0, 34], 1000),
                "Obfuscation" => ([136, 136, 136], 1000),
                "Potential" => ([255, 255, 255], 600),
                "Entanglement" => ([102, 51, 0], 1000),
                "Repulsion" => ([255, 255, 255], 1000),
                "Camouflage" => ([68, 136, 34], 1000),
                "Affection" => ([255, 68, 136], 1000),
                "Anti-Magic" => ([136, 238, 255], 1000),
                _ => return None,
            }
        } else if cursed || effect.is_some_and(Effect::is_curse) {
            ([0, 0, 0], 1000)
        } else {
            return None;
        };
        Some(Self { color, period_ms })
    }
}
