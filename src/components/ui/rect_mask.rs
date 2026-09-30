//! src/components/ui/rect_mask.rs — RectMask: clip descendants to a rect (#418).
//!
//! Unity's `RectMask2D`. Every graphic on this entity and below it is clipped to
//! the axis-aligned screen bounds of this element's rect, inset by `padding`;
//! nested masks intersect. The renderer applies it as a scissor rect, so the
//! clip is rectangular even when the masked element is rotated (as in Unity).
//! Soft and shape masks are #428. Pure authoring data.

use glam::Vec4;
use serde::{Deserialize, Serialize};

/// A rectangular clip over a UI subtree. See the module docs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RectMaskComponent {
    /// Inset of the clip from the element's bounds in reference units — `x` left,
    /// `y` bottom, `z` right, `w` top (Unity's order). Negative grows the clip.
    pub padding: Vec4,
}
