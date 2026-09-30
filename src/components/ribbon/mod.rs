//! src/components/ribbon/ — the TrailRenderer and LineRenderer components (#441).
//!
//! Both draw a **ribbon**: a camera-facing strip through a list of points, whose
//! width and colour vary along its length. They differ only in where the points
//! come from — a [`TrailComponent`] records its entity's path over a time window
//! (rocket smoke, bullet tracers, a melee swipe), a [`LineComponent`] holds points
//! a script or the inspector set (laser sights, grenade-lineup arcs, taser beams).
//! The look they share is one [`RibbonStyle`], so one inspector section, one set of
//! API verbs and one render pass (`render::passes::ribbons`) serve both.
//!
//! Width and colour are the #439 [`Curve`] and [`Gradient`] over the ribbon's
//! normalised length: `t = 0` at the start (a trail's head, at the entity; a line's
//! first point), `t = 1` at the end.

mod line;
mod trail;

pub use line::LineComponent;
pub use trail::{TrailComponent, TrailPoint, TrailRuntime};

use serde::{Deserialize, Serialize};

use crate::components::ParticleBlend;
use crate::core::curve::{Curve, Gradient};

/// How the texture's `u` coordinate runs along the ribbon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureMode {
    /// One copy of the texture stretched over the whole length.
    #[default]
    Stretch,
    /// The texture repeats once per world unit of length.
    Tile,
}

impl TextureMode {
    /// The mode's name, as the API spells it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Stretch => "Stretch",
            Self::Tile => "Tile",
        }
    }

    /// Parse a mode name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "stretch" => Some(Self::Stretch),
            "tile" => Some(Self::Tile),
            _ => None,
        }
    }
}

/// The look a trail and a line share: width and colour along the length, the
/// texture and how it maps, and the blend (as particles: alpha or additive).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RibbonStyle {
    /// World-space width along the normalised length.
    pub width: Curve,
    /// RGBA along the normalised length.
    pub color: Gradient,
    /// Optional texture path; the renderer's default white texture without one.
    #[serde(default)]
    pub texture: Option<String>,
    #[serde(default)]
    pub texture_mode: TextureMode,
    pub blend: ParticleBlend,
}

impl RibbonStyle {
    /// A constant-width, constant-colour style.
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
