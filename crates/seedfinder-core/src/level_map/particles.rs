//! Portable, continuous ambient-particle primitives. Coordinates in milli-pixels
//! preserve sub-pixel motion without floating-point fields in map documents.
use super::{MapBlend, MapDraw};

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
pub struct MapCurve {
    /// Piecewise-linear [lifetime progress, value], both divided by 1000.
    pub points: Vec<[u16; 2]>,
    /// Apply sqrt after interpolation (the game's question-mark pulse).
    pub sqrt: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapParticle {
    pub birth_ms: u16,
    pub angle: u16,
    pub lifespan_ms: u16,
    pub position: [i32; 2],
    /// Initial image scale, divided by 1000.
    pub scale: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "json-query", derive(serde::Serialize))]
#[cfg_attr(feature = "json-query", serde(rename_all = "camelCase"))]
pub struct MapEmitter {
    /// Delayed first emission. Absent means the ambient loop is prewarmed.
    #[cfg_attr(feature = "json-query", serde(skip_serializing_if = "Option::is_none"))]
    pub start_ms: Option<u32>,
    /// Foreground terrain occludes world particles; status icons stay above it.
    pub wall_mask: bool,
    pub cell: usize,
    pub loop_ms: u16,
    pub blend: Option<MapBlend>,
    /// Sprite/fill centered at each particle's position, independent of cells.
    pub image: MapDraw,
    /// Pixels/second and pixels/second².
    pub velocity: [i16; 2],
    pub angular_speed: i16,
    pub acceleration: [i16; 2],
    pub alpha: MapCurve,
    pub scale: MapCurve,
    pub particles: Vec<MapParticle>,
}
