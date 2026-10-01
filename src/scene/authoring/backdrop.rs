//! src/scene/authoring/backdrop.rs — Shared BackdropFilter-authoring ops (#426).
//!
//! The ONE place the engine mutates an entity's first-class
//! `BackdropFilterComponent`. The editor's Backdrop Filter card and the Lua
//! `BackdropFilter.*` namespace both route every write through this.
//!
//! Allowed deps: components (the `BackdropFilterComponent` data). Pure.

use glam::Vec4;

use crate::components::BackdropFilterComponent;

/// Set the blur radius in reference units, clamped to `>= 0`.
pub fn set_blur_radius(b: &mut BackdropFilterComponent, radius: f32) {
    b.blur_radius = radius.max(0.0);
}

/// Set the tint mixed over the backdrop (RGBA; alpha is the mix), clamped to `[0, 1]`.
pub fn set_tint(b: &mut BackdropFilterComponent, tint: Vec4) {
    b.tint = tint.clamp(Vec4::ZERO, Vec4::ONE);
}

/// Set the saturation (`1` unchanged, `0` greyscale), clamped to `>= 0`.
pub fn set_saturation(b: &mut BackdropFilterComponent, saturation: f32) {
    b.saturation = saturation.max(0.0);
}

/// Set the brightness multiplier, clamped to `>= 0`.
pub fn set_brightness(b: &mut BackdropFilterComponent, brightness: f32) {
    b.brightness = brightness.max(0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut b = BackdropFilterComponent::default();
        set_blur_radius(&mut b, -1.0);
        assert_eq!(b.blur_radius, 0.0);
        set_tint(&mut b, Vec4::new(2.0, 0.5, -1.0, 0.25));
        assert_eq!(b.tint, Vec4::new(1.0, 0.5, 0.0, 0.25));
        set_saturation(&mut b, -2.0);
        set_brightness(&mut b, 0.4);
        assert_eq!((b.saturation, b.brightness), (0.0, 0.4));
    }
}
