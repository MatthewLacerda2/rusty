//! src/components/ui/image.rs — Image: the UI's textured / solid rectangle (#418).
//!
//! Unity uGUI's `Image`. It fills its entity's laid-out rect with a tint `color`,
//! optionally multiplied by a `texture` (a path, like material maps; `None` draws a
//! solid colour). `image_type` picks how the texture maps onto the rect:
//!
//! - `Simple` — stretched over the rect (`preserve_aspect` letterboxes it instead);
//! - `Sliced` — 9-slice: the `border` texels keep their size, the centre stretches;
//! - `Tiled` — repeated at native size from the rect's bottom-left;
//! - `Filled` — only `fill_amount` of the rect shows, swept by `fill_method` from
//!   `fill_origin` (health bars, cooldown rings, reload circles).
//!
//! One texel is one reference unit (Unity's 100 pixels-per-unit sprite on a
//! 100 reference-pixels-per-unit canvas). Colours are in display (sRGB-encoded)
//! space with straight alpha, as a designer picks them. A `gradient` replaces the
//! flat tint and `blend` picks the compositing mode (#425, `look`). Authoring data, plus two
//! runtime-only slots a `Selectable` drives (never serialized).

use glam::Vec4;
use serde::{Deserialize, Serialize};

use super::look::{UiBlend, UiGradient};

/// How the texture maps onto the rect. See the module docs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageType {
    /// Stretched over the whole rect.
    #[default]
    Simple,
    /// 9-slice by `border`.
    Sliced,
    /// Repeated at native size.
    Tiled,
    /// Partially shown by `fill_method` / `fill_origin` / `fill_amount`.
    Filled,
}

/// The shape a `Filled` image sweeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillMethod {
    /// A bar filling left↔right.
    #[default]
    Horizontal,
    /// A bar filling bottom↔top.
    Vertical,
    /// A full-circle sweep around the rect's centre.
    Radial360,
}

/// Where a fill starts. `Horizontal` uses `Left` / `Right`, `Vertical` uses
/// `Bottom` / `Top`, `Radial360` any of the four (the edge its sweep starts at).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FillOrigin {
    #[default]
    Left,
    Right,
    Bottom,
    Top,
}

/// A UI graphic. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageComponent {
    /// RGBA tint in display space, straight alpha, each channel in `[0, 1]`.
    pub color: Vec4,
    /// The texture's path; `None` draws a solid `color`.
    pub texture: Option<String>,
    /// How the texture maps onto the rect.
    pub image_type: ImageType,
    /// `Sliced` borders in texels — `x` left, `y` bottom, `z` right, `w` top.
    pub border: Vec4,
    /// `Filled`: the sweep's shape.
    pub fill_method: FillMethod,
    /// `Filled`: where the sweep starts.
    pub fill_origin: FillOrigin,
    /// `Filled`: the visible fraction, in `[0, 1]`.
    pub fill_amount: f32,
    /// `Filled` + `Radial360`: sweep clockwise (else counter-clockwise).
    pub fill_clockwise: bool,
    /// `Simple`: keep the texture's aspect, centred inside the rect.
    pub preserve_aspect: bool,
    /// Whether the pointer can hit this graphic (read by #420).
    pub raycast_target: bool,
    /// A gradient tint in place of `color` (#425), multiplying the texture.
    pub gradient: Option<UiGradient>,
    /// How it composites onto the frame (#425).
    pub blend: UiBlend,
    /// Runtime only, never saved: the colour a `Selectable`'s `ColorTint` multiplies
    /// in (Unity's `CanvasRenderer` colour). Written by the event system (#420).
    #[serde(skip)]
    pub state_tint: Vec4,
    /// Runtime only, never saved: the sprite a `Selectable`'s `SpriteSwap` shows in
    /// place of `texture` (Unity's `Image.overrideSprite`, #420).
    #[serde(skip)]
    pub override_texture: Option<String>,
}

impl Default for ImageComponent {
    fn default() -> Self {
        Self {
            color: Vec4::ONE,
            texture: None,
            image_type: ImageType::Simple,
            border: Vec4::ZERO,
            fill_method: FillMethod::Horizontal,
            fill_origin: FillOrigin::Left,
            fill_amount: 1.0,
            fill_clockwise: true,
            preserve_aspect: false,
            raycast_target: true,
            gradient: None,
            blend: UiBlend::Normal,
            state_tint: Vec4::ONE,
            override_texture: None,
        }
    }
}

impl ImageComponent {
    /// The texture this image draws: a `SpriteSwap` override, else its own.
    pub fn shown_texture(&self) -> Option<&str> {
        self.override_texture.as_deref().or(self.texture.as_deref())
    }
}
