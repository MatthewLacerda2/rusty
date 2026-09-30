//! src/scene/authoring/canvas.rs — Shared canvas-authoring ops (#417).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `CanvasComponent` field by field. The editor's Canvas card and the Lua `Canvas.*`
//! namespace both route every write through these, so validation lives once: the
//! reference resolution is kept ≥ 1 per axis and the width/height match in `[0, 1]`.
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

/// Parse a render-mode name (case-insensitive). `None` for an unknown name.
pub fn parse_render_mode(name: &str) -> Option<CanvasRenderMode> {
    match name.to_lowercase().as_str() {
        "screenspaceoverlay" | "overlay" => Some(CanvasRenderMode::ScreenSpaceOverlay),
        _ => None,
    }
}

/// The render mode's Unity name.
pub fn render_mode_name(mode: CanvasRenderMode) -> &'static str {
    match mode {
        CanvasRenderMode::ScreenSpaceOverlay => "ScreenSpaceOverlay",
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
        let mode = CanvasRenderMode::ScreenSpaceOverlay;
        assert_eq!(parse_render_mode(render_mode_name(mode)), Some(mode));
        assert_eq!(parse_render_mode("WorldSpace"), None);
    }
}
