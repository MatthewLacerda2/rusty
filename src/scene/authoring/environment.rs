//! src/scene/authoring/environment.rs — Shared scene-environment ops (#1612).
//!
//! The ONE place the engine writes the scene's environment scalars: the skybox
//! panorama and the ambient light (Unity's `RenderSettings.skybox`, `ambientLight`
//! and `ambientIntensity`). BOTH the editor's Scene Settings panel and the Lua
//! `Lighting.Set*` setters route through these, so panel and binding share one
//! write and one clamp. The renderer picks a change up on its next frame: a new
//! panorama is loaded and its sky reflection cube baked (#1553), and the ambient
//! colour is packed into every pass's lighting uniform.
//!
//! Pure.

use glam::Vec3;

use super::sanitize::{finite3_or, non_negative};
use crate::scene::Scene;

/// Set the skybox panorama's path, as the scene file stores it; an empty path goes
/// back to the procedural sky. A path that loads nothing draws the renderer's
/// fallback texture, as a hand-edited scene file's would.
pub fn set_skybox(scene: &mut Scene, path: &str) {
    scene.skybox_path = path.to_string();
}

/// Set the ambient colour (linear RGB), each channel clamped to `≥ 0`. A colour
/// with a NaN or infinite channel is refused whole and changes nothing.
pub fn set_ambient_color(scene: &mut Scene, color: Vec3) {
    scene.ambient_color = finite3_or(color, scene.ambient_color).max(Vec3::ZERO);
}

/// Set the ambient intensity, clamped to `≥ 0`.
pub fn set_ambient_intensity(scene: &mut Scene, intensity: f32) {
    scene.ambient_intensity = non_negative(intensity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_with_clamps() {
        let mut scene = Scene::new();
        set_skybox(&mut scene, "sky/day.hdr");
        set_ambient_color(&mut scene, Vec3::new(-1.0, 0.5, 2.0));
        set_ambient_color(&mut scene, Vec3::new(1.0, f32::NAN, 1.0));
        set_ambient_intensity(&mut scene, -0.5);
        assert_eq!(scene.skybox_path, "sky/day.hdr");
        assert_eq!(
            scene.ambient_color,
            Vec3::new(0.0, 0.5, 2.0),
            "NaN keeps it"
        );
        assert_eq!(scene.ambient_intensity, 0.0);
        set_ambient_intensity(&mut scene, f32::INFINITY);
        assert_eq!(scene.ambient_intensity, 0.0, "infinite is refused");
        set_skybox(&mut scene, "");
        assert!(scene.skybox_path.is_empty(), "empty is the procedural sky");
    }
}
