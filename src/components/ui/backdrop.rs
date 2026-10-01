//! src/components/ui/backdrop.rs — BackdropFilter: frosted glass behind a graphic (#426).
//!
//! CSS's `backdrop-filter`, for a UI panel: the 3D frame behind this entity's
//! graphic is blurred, desaturated, brightened and tinted, and shown through the
//! graphic's shape (its `Image`'s texture alpha, else its `Shape`, else its rect)
//! under the graphic itself — so a pause menu sits on a blurred, darkened game. The graphic's
//! own colour alpha does not hide the backdrop: an `Image` with alpha 0 is pure
//! frosted glass. Screen-space (overlay) canvases only; `docs/ui.md` says why.
//! Pure authoring data.

use glam::Vec4;
use serde::{Deserialize, Serialize};

/// The filter applied to what is behind a UI graphic. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackdropFilterComponent {
    /// Blur radius in reference units (scaled with the canvas like every UI length).
    /// `0` shows the backdrop unblurred. Kept `>= 0`.
    pub blur_radius: f32,
    /// A colour mixed over the blurred backdrop by its alpha (`0`: no tint).
    pub tint: Vec4,
    /// `1` keeps the colours, `0` is greyscale, above `1` oversaturates. Kept `>= 0`.
    pub saturation: f32,
    /// Multiplies the backdrop (`< 1` darkens). Kept `>= 0`.
    pub brightness: f32,
}

impl Default for BackdropFilterComponent {
    fn default() -> Self {
        Self {
            blur_radius: 16.0,
            tint: Vec4::ZERO,
            saturation: 1.0,
            brightness: 1.0,
        }
    }
}
