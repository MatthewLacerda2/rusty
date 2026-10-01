//! src/components/ui/look.rs — the look every UI graphic shares: blend and gradient (#425).
//!
//! [`UiBlend`] is how a graphic composites onto what is under it — on `Image`,
//! `Text` and `Shape`. The UI blends in display (sRGB-encoded) space with
//! premultiplied alpha, so the four modes read as an image editor's layer modes:
//! `Additive` for neon, glows and hit flashes, `Multiply` to darken, `Screen` to
//! lighten. A change of blend mode starts a new draw batch.
//!
//! [`UiGradient`] replaces a graphic's flat colour with 2–4 colour stops swept
//! across its rect — linearly at an angle, or radially from a centre. It is the
//! tint of an `Image` and the fill of a `Shape`. Positions are fractions of the
//! rect, so a gradient scales with the element. Pure authoring data.

use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

/// How a graphic composites onto the frame. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UiBlend {
    /// Ordinary alpha blending ("over").
    #[default]
    Normal,
    /// Adds the colour: brightens, never darkens (neon, glows, flashes).
    Additive,
    /// Multiplies the colour under it: darkens (white leaves it unchanged).
    Multiply,
    /// The inverse of multiplying the inverses: lightens (black leaves it unchanged).
    Screen,
}

impl UiBlend {
    /// Every mode, in declaration order.
    pub const ALL: [UiBlend; 4] = [
        UiBlend::Normal,
        UiBlend::Additive,
        UiBlend::Multiply,
        UiBlend::Screen,
    ];
}

/// How a gradient sweeps across the rect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientKind {
    /// Along a direction at `angle` degrees, corner to corner.
    #[default]
    #[serde(alias = "linear")]
    Linear,
    /// Outward from `center`, reaching the last stop at `radius`.
    #[serde(alias = "radial")]
    Radial,
}

/// One colour stop: a straight-alpha display-space colour at position `t` in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    /// Where along the gradient, in `[0, 1]`.
    pub t: f32,
    /// RGBA, display space, straight alpha.
    pub color: Vec4,
}

/// The most stops a gradient carries (the vertex format's room).
pub const MAX_GRADIENT_STOPS: usize = 4;

/// A 2–4 stop colour gradient over a rect. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiGradient {
    /// Linear or radial.
    pub kind: GradientKind,
    /// `Linear`: the direction in degrees, counter-clockwise from +x (`0` runs left
    /// → right, `90` bottom → top).
    pub angle: f32,
    /// `Radial`: the centre as a fraction of the rect (`0.5, 0.5` is its middle).
    pub center: Vec2,
    /// `Radial`: the distance (a fraction of the rect, per axis) the last stop
    /// sits at — `0.5` reaches the middle of each edge.
    pub radius: f32,
    /// The stops, in ascending `t` (kept so by the authoring ops).
    pub stops: Vec<GradientStop>,
}

impl Default for UiGradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            angle: 0.0,
            center: Vec2::splat(0.5),
            radius: 0.5,
            stops: vec![
                GradientStop {
                    t: 0.0,
                    color: Vec4::ONE,
                },
                GradientStop {
                    t: 1.0,
                    color: Vec4::new(0.0, 0.0, 0.0, 1.0),
                },
            ],
        }
    }
}
