//! src/scene/authoring/fog.rs — Shared scene-fog ops (#437).
//!
//! The ONE place the engine mutates the scene's `FogSettings`, with the clamps
//! each write owns. BOTH the editor's Scene Settings panel and the Lua
//! `Graphics.SetFog*` setters route through these, so the panel and the binding
//! share one write + one clamp. The `Graphics` adapter still parses the mode name
//! into the typed value `set_mode` takes.
//!
//! Allowed deps: scene (the `FogSettings` data). Pure.

use glam::Vec3;

use crate::scene::fog::{FogMode, FogSettings};

/// Set the fog mode (`Off` disables fog everywhere).
pub fn set_mode(fog: &mut FogSettings, mode: FogMode) {
    fog.mode = mode;
}

/// Set the fog colour (linear RGB), each channel clamped to `≥ 0`.
pub fn set_color(fog: &mut FogSettings, color: Vec3) {
    fog.color = color.max(Vec3::ZERO);
}

/// Set the exponential density, clamped to `≥ 0`.
pub fn set_density(fog: &mut FogSettings, density: f32) {
    fog.density = density.max(0.0);
}

/// Set where fog begins, clamped to `≥ 0`.
pub fn set_start(fog: &mut FogSettings, start: f32) {
    fog.start = start.max(0.0);
}

/// Set where linear fog is full, clamped to `≥ 0`. An end at or before the start
/// fogs everything past the start fully; the shader guards the divide.
pub fn set_end(fog: &mut FogSettings, end: f32) {
    fog.end = end.max(0.0);
}

/// Set how fast fog thins with height, clamped to `≥ 0` (`0` is uniform fog).
pub fn set_height_falloff(fog: &mut FogSettings, falloff: f32) {
    fog.height_falloff = falloff.max(0.0);
}

/// Set the height below which fog is at full thickness.
pub fn set_base_height(fog: &mut FogSettings, height: f32) {
    fog.base_height = height;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_with_clamps() {
        let mut fog = FogSettings::default();
        set_mode(&mut fog, FogMode::ExponentialSquared);
        set_color(&mut fog, Vec3::new(-1.0, 0.5, 2.0));
        set_density(&mut fog, -0.1);
        set_start(&mut fog, -5.0);
        set_end(&mut fog, -1.0);
        set_height_falloff(&mut fog, -2.0);
        set_base_height(&mut fog, -3.0);
        assert_eq!(fog.mode, FogMode::ExponentialSquared);
        assert_eq!(fog.color, Vec3::new(0.0, 0.5, 2.0));
        assert_eq!((fog.density, fog.start, fog.end), (0.0, 0.0, 0.0));
        assert_eq!(fog.height_falloff, 0.0);
        assert_eq!(fog.base_height, -3.0, "base height is any world height");
    }
}
