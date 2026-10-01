//! src/scene/authoring/rect_mask.rs — Shared RectMask-authoring ops (#418).
//!
//! The ONE place the engine mutates an entity's first-class `RectMaskComponent`
//! (and, #428, its graphic-shaped sibling `MaskComponent`).
//! The editor's Rect Mask and Mask cards and the Lua `RectMask.*` / `Mask.*`
//! namespaces route every write through this.
//!
//! Allowed deps: components (the `RectMaskComponent` data). Pure.

use glam::Vec4;

use crate::components::{MaskComponent, RectMaskComponent};

/// Set the clip's inset from the element's bounds (left, bottom, right, top), in
/// reference units. Negative values grow the clip past the rect.
pub fn set_padding(m: &mut RectMaskComponent, padding: Vec4) {
    m.padding = padding;
}

/// Set the soft edge's width in reference units, clamped to `>= 0` (`0`: hard).
pub fn set_feather(m: &mut RectMaskComponent, feather: f32) {
    m.feather = feather.max(0.0);
}

/// Set whether a Mask also draws its own graphic.
pub fn set_show_mask_graphic(m: &mut MaskComponent, show: bool) {
    m.show_mask_graphic = show;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_writes_through() {
        let mut m = RectMaskComponent::default();
        set_padding(&mut m, Vec4::new(1.0, -2.0, 3.0, 4.0));
        assert_eq!(m.padding, Vec4::new(1.0, -2.0, 3.0, 4.0));
        set_feather(&mut m, -3.0);
        assert_eq!(m.feather, 0.0);
        set_feather(&mut m, 6.0);
        assert_eq!(m.feather, 6.0);
    }

    #[test]
    fn mask_graphic_toggle_writes_through() {
        let mut m = MaskComponent::default();
        assert!(m.show_mask_graphic);
        set_show_mask_graphic(&mut m, false);
        assert!(!m.show_mask_graphic);
    }
}
