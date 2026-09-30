//! src/scene/authoring/canvas.rs — Shared canvas-authoring ops (#417).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `CanvasComponent` field by field. The editor's Canvas card and the Lua `Canvas.*`
//! namespace both route every write through these, so validation lives once: the
//! reference resolution is kept ≥ 1 per axis, the width/height match and the sway
//! in `[0, 1]`, the world density > 0 and the camera plane ≥ 1 cm away (#429).
//!
//! Allowed deps: components (the `CanvasComponent` data). Pure.

use glam::Vec2;

use crate::components::{CanvasComponent, CanvasRenderMode};

/// Set where the canvas draws.
pub fn set_render_mode(c: &mut CanvasComponent, mode: CanvasRenderMode) {
    c.render_mode = mode;
}

/// Set the cross-canvas draw / hit-test order (higher is on top).
pub fn set_sort_order(c: &mut CanvasComponent, sort_order: i32) {
    c.sort_order = sort_order;
}

/// Set the authoring resolution, each axis clamped to at least 1.
pub fn set_reference_resolution(c: &mut CanvasComponent, resolution: Vec2) {
    c.reference_resolution = resolution.max(Vec2::ONE);
}

/// Set the width (0) ↔ height (1) scale match, clamped to `[0, 1]`.
pub fn set_match_width_or_height(c: &mut CanvasComponent, value: f32) {
    c.match_width_or_height = value.clamp(0.0, 1.0);
}

/// Set the `WorldSpace` density (reference units per metre), kept above zero.
pub fn set_pixels_per_unit(c: &mut CanvasComponent, value: f32) {
    c.pixels_per_unit = if value.is_finite() {
        value.max(1e-3)
    } else {
        1e-3
    };
}

/// Set the `ScreenSpaceCamera` plane's distance in metres, kept ≥ 0.01.
pub fn set_plane_distance(c: &mut CanvasComponent, value: f32) {
    c.plane_distance = if value.is_finite() {
        value.max(0.01)
    } else {
        0.01
    };
}

/// Set the `ScreenSpaceCamera` tilt in degrees, each axis clamped to ±89.
pub fn set_tilt(c: &mut CanvasComponent, tilt: Vec2) {
    c.tilt = tilt.clamp(Vec2::splat(-89.0), Vec2::splat(89.0));
}

/// Set the `ScreenSpaceCamera` sway amount, clamped to `[0, 1]`.
pub fn set_sway(c: &mut CanvasComponent, value: f32) {
    c.sway = value.clamp(0.0, 1.0);
}

/// Every render mode, in menu order.
pub const RENDER_MODES: [CanvasRenderMode; 3] = [
    CanvasRenderMode::ScreenSpaceOverlay,
    CanvasRenderMode::ScreenSpaceCamera,
    CanvasRenderMode::WorldSpace,
];

/// Parse a render-mode name (case-insensitive). `None` for an unknown name.
pub fn parse_render_mode(name: &str) -> Option<CanvasRenderMode> {
    match name.to_lowercase().as_str() {
        "screenspaceoverlay" | "overlay" => Some(CanvasRenderMode::ScreenSpaceOverlay),
        "screenspacecamera" | "camera" => Some(CanvasRenderMode::ScreenSpaceCamera),
        "worldspace" | "world" => Some(CanvasRenderMode::WorldSpace),
        _ => None,
    }
}

/// The render mode's Unity name.
pub fn render_mode_name(mode: CanvasRenderMode) -> &'static str {
    match mode {
        CanvasRenderMode::ScreenSpaceOverlay => "ScreenSpaceOverlay",
        CanvasRenderMode::ScreenSpaceCamera => "ScreenSpaceCamera",
        CanvasRenderMode::WorldSpace => "WorldSpace",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_write_through_and_clamp() {
        let mut c = CanvasComponent::default();
        set_sort_order(&mut c, 7);
        set_reference_resolution(&mut c, Vec2::new(0.0, 720.0));
        set_match_width_or_height(&mut c, 3.0);
        set_render_mode(&mut c, CanvasRenderMode::ScreenSpaceOverlay);
        assert_eq!(c.sort_order, 7);
        assert_eq!(c.reference_resolution, Vec2::new(1.0, 720.0));
        assert_eq!(c.match_width_or_height, 1.0);
        set_match_width_or_height(&mut c, -1.0);
        assert_eq!(c.match_width_or_height, 0.0);
    }

    #[test]
    fn render_mode_names_round_trip() {
        for mode in RENDER_MODES {
            assert_eq!(parse_render_mode(render_mode_name(mode)), Some(mode));
        }
        assert_eq!(parse_render_mode("Holographic"), None);
    }

    #[test]
    fn world_and_camera_fields_clamp() {
        let mut c = CanvasComponent::default();
        set_pixels_per_unit(&mut c, -5.0);
        set_plane_distance(&mut c, 0.0);
        set_tilt(&mut c, Vec2::new(120.0, -10.0));
        set_sway(&mut c, 2.0);
        assert_eq!(c.pixels_per_unit, 1e-3);
        assert_eq!(c.plane_distance, 0.01);
        assert_eq!(c.tilt, Vec2::new(89.0, -10.0));
        assert_eq!(c.sway, 1.0);
        set_pixels_per_unit(&mut c, 250.0);
        assert_eq!(c.pixels_per_unit, 250.0);
    }
}
