//! src/components/ui/mask.rs — Mask: clip descendants to this entity's graphic (#428).
//!
//! Unity's `Mask`. Every graphic *below* this entity is clipped to the coverage of
//! this entity's own graphic — its `Image`'s texture alpha times its colour alpha,
//! else its `Shape`'s coverage, else its rect — so an ellipse or a circle sprite
//! makes a round minimap and a soft-edged sprite a feathered one. Nested masks multiply. The mask
//! graphic itself draws only when `show_mask_graphic` is on. The renderer applies
//! it as a mask texture sampled per fragment (`docs/ui.md`). Pure authoring data.

use serde::{Deserialize, Serialize};

/// A graphic-shaped clip over a UI subtree. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaskComponent {
    /// Draw the mask's own graphic as well as clipping with it (Unity's
    /// `showMaskGraphic`). Off: the graphic only shapes the clip.
    pub show_mask_graphic: bool,
}

impl Default for MaskComponent {
    fn default() -> Self {
        Self {
            show_mask_graphic: true,
        }
    }
}
