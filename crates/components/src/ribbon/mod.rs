//! A **ribbon**: a camera-facing strip through points, width and colour varying along it.
//! A trail records its entity's path; a line holds authored points. [`RibbonStyle`] is shared.
//! Width and colour are a [`Curve`] and [`Gradient`] over normalised length;
//! `t = 0` is the start (a trail's head, a line's first point), `t = 1` the end.

mod line;
mod trail;

pub use line::LineComponent;
pub use trail::{TrailComponent, TrailPoint, TrailRuntime};

use serde::{Deserialize, Serialize};

use crate::ParticleBlend;
use rusty_asset::MapRef;
use rusty_core::curve::{Curve, Gradient};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextureMode {
        #[default]
        Stretch = "Stretch",
        Tile = "Tile",
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RibbonStyle {
    pub width: Curve,
    pub color: Gradient,
    /// Optional texture path; the renderer's default white texture without one.
    #[serde(default)]
    pub texture: Option<MapRef>,
    #[serde(default)]
    pub texture_mode: TextureMode,
    pub blend: ParticleBlend,
}

impl RibbonStyle {
    pub fn solid(width: f32, color: [f32; 4]) -> Self {
        Self {
            width: Curve::constant(width),
            color: Gradient::solid(color),
            texture: None,
            texture_mode: TextureMode::Stretch,
            blend: ParticleBlend::Alpha,
        }
    }
}

impl Default for RibbonStyle {
    fn default() -> Self {
        Self::solid(0.1, [1.0; 4])
    }
}
