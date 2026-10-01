//! src/components/ui/shape.rs — Shape: a texture-free SDF graphic (#425).
//!
//! A primitive drawn from a signed distance field, so it stays crisp at any scale
//! and needs no bitmap — the thin cut-corner frames, neon lines, rings and soft
//! glows a designed HUD is mostly made of. It fills its entity's laid-out rect:
//!
//! - `Rect` — the rect, with per-corner `radius` that either rounds the corners
//!   (`corner = Round`) or cuts them off at 45° (`corner = Chamfer`);
//! - `Ellipse` — the ellipse inscribed in the rect;
//! - `Ring` — a circle of the rect's smaller half-extent with a hole of
//!   `inner_radius`, swept from `arc_start` to `arc_end` (crosshair arcs, radial
//!   progress);
//! - `Line` — a `thickness`-thick line along the rect's horizontal centre line,
//!   optionally dashed (`dash` on, `gap` off). Rotate the RectTransform to aim it.
//!
//! The fill is `color` or, when set, a `gradient`; `border_width` draws a
//! `border_color` band inside the edge (the outline). `shadow` and `glow` are
//! effects cut from the same distance field: a soft offset copy under the shape,
//! and a halo fading out from its edge. All lengths are reference units; colours
//! are display space, straight alpha. Authoring data, plus one runtime-only slot a
//! `Selectable` drives (never serialized).

use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

use super::look::{UiBlend, UiGradient};
use super::shader::UiShader;

/// The primitive a Shape draws. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    /// The rect, corners rounded or chamfered by `radius`.
    #[default]
    Rect,
    /// The inscribed ellipse.
    Ellipse,
    /// A circular band, optionally an arc.
    Ring,
    /// A straight, optionally dashed line.
    Line,
}

/// What a `Rect`'s per-corner `radius` does to its corners.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeCorner {
    /// Quarter-circle corners of `radius`.
    #[default]
    Round,
    /// Corners cut off at 45°, `radius` along each edge.
    Chamfer,
}

/// A soft copy of the shape under it. Off while `color.w` is 0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeShadow {
    /// Where the shadow sits relative to the shape, reference units (y up).
    pub offset: Vec2,
    /// How far its edge softens either side, reference units (0: hard).
    pub blur: f32,
    /// RGBA, display space, straight alpha.
    pub color: Vec4,
}

impl Default for ShapeShadow {
    fn default() -> Self {
        Self {
            offset: Vec2::new(4.0, -4.0),
            blur: 4.0,
            color: Vec4::ZERO,
        }
    }
}

/// A halo fading out from the shape's edge. Off while `size` or `color.w` is 0.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeGlow {
    /// How far it reaches past the edge, reference units.
    pub size: f32,
    /// Brightness multiplier on `color` (above 1 glows hotter, best with `Additive`).
    pub intensity: f32,
    /// RGBA, display space, straight alpha.
    pub color: Vec4,
}

impl Default for ShapeGlow {
    fn default() -> Self {
        Self {
            size: 0.0,
            intensity: 1.0,
            color: Vec4::ZERO,
        }
    }
}

/// A texture-free SDF UI graphic. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShapeComponent {
    /// Which primitive.
    pub kind: ShapeKind,
    /// `Rect`: round or chamfer the corners.
    pub corner: ShapeCorner,
    /// `Rect`: per-corner size — `x` top-left, `y` top-right, `z` bottom-right,
    /// `w` bottom-left (CSS order). Kept within half the rect's smaller side.
    pub radius: Vec4,
    /// `Ring`: the hole's radius (0: a full disc or pie).
    pub inner_radius: f32,
    /// `Ring`: where the arc starts, degrees clockwise from 12 o'clock.
    pub arc_start: f32,
    /// `Ring`: where it ends; a sweep of 360° or more is the whole ring.
    pub arc_end: f32,
    /// `Line`: its thickness.
    pub thickness: f32,
    /// `Line`: each dash's length (0: solid).
    pub dash: f32,
    /// `Line`: the gap between dashes.
    pub gap: f32,
    /// Fill RGBA, display space, straight alpha (ignored while `gradient` is set).
    pub color: Vec4,
    /// A gradient fill in place of `color`.
    pub gradient: Option<UiGradient>,
    /// The border band's width inside the edge (0: none).
    pub border_width: f32,
    /// The border's RGBA.
    pub border_color: Vec4,
    /// Drop shadow.
    pub shadow: ShapeShadow,
    /// Outer glow.
    pub glow: ShapeGlow,
    /// How it composites onto the frame.
    pub blend: UiBlend,
    /// Whether the pointer can hit this graphic (its rect, as for `Image`).
    pub raycast_target: bool,
    /// A custom ui shader to draw with (#427); `None` draws with the standard one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shader: Option<UiShader>,
    /// Runtime only, never saved: the colour a `Selectable`'s `ColorTint` multiplies
    /// in (#420).
    #[serde(skip)]
    pub state_tint: Vec4,
}

impl Default for ShapeComponent {
    fn default() -> Self {
        Self {
            kind: ShapeKind::Rect,
            corner: ShapeCorner::Round,
            radius: Vec4::ZERO,
            inner_radius: 0.0,
            arc_start: 0.0,
            arc_end: 360.0,
            thickness: 2.0,
            dash: 0.0,
            gap: 0.0,
            color: Vec4::ONE,
            gradient: None,
            border_width: 0.0,
            border_color: Vec4::ONE,
            shadow: ShapeShadow::default(),
            glow: ShapeGlow::default(),
            blend: UiBlend::Normal,
            raycast_target: true,
            shader: None,
            state_tint: Vec4::ONE,
        }
    }
}

impl ShapeShadow {
    /// Whether it draws anything.
    pub fn is_on(&self) -> bool {
        self.color.w > 0.0
    }
}

impl ShapeGlow {
    /// Whether it draws anything.
    pub fn is_on(&self) -> bool {
        self.size > 0.0 && self.color.w > 0.0 && self.intensity > 0.0
    }
}
