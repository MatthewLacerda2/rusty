//! src/scene/authoring/rect_mask.rs — Shared RectMask-authoring ops (#418).
//!
//! The ONE place the engine mutates an entity's first-class `RectMaskComponent`.
//! The editor's Rect Mask card and the Lua `RectMask.*` namespace both route every
//! write through this.
//!
//! Allowed deps: components (the `RectMaskComponent` data). Pure.

use glam::Vec4;

use crate::components::RectMaskComponent;

/// Set the clip's inset from the element's bounds (left, bottom, right, top), in
/// reference units. Negative values grow the clip past the rect.
pub fn set_padding(m: &mut RectMaskComponent, padding: Vec4) {
    m.padding = padding;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_writes_through() {
        let mut m = RectMaskComponent::default();
        set_padding(&mut m, Vec4::new(1.0, -2.0, 3.0, 4.0));
        assert_eq!(m.padding, Vec4::new(1.0, -2.0, 3.0, 4.0));
    }
}
