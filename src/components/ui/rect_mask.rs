//! src/components/ui/rect_mask.rs — RectMask: clip descendants to a rect (#418).
//!
//! Unity's `RectMask2D`. Every graphic on this entity and below it is clipped to
//! the axis-aligned screen bounds of this element's rect, inset by `padding`;
//! nested masks intersect. The renderer applies it as a scissor rect, so the
//! clip is rectangular even when the masked element is rotated (as in Unity).
//! `feather` softens its edges (#428) — faded list ends; graphic-shaped clips are
//! `MaskComponent`. Pure authoring data.

use glam::Vec4;
use serde::{Deserialize, Serialize};

/// A rectangular clip over a UI subtree. See the module docs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RectMaskComponent {
    /// Inset of the clip from the element's bounds in reference units — `x` left,
    /// `y` bottom, `z` right, `w` top (Unity's order). Negative grows the clip.
    pub padding: Vec4,
    /// Width of the soft edge, reference units (#428): content fades out over this
    /// distance inside the clip's edge. `0` is a hard clip. Kept `>= 0`.
    pub feather: f32,
}
