//! src/components/ui/canvas.rs — Canvas component: the root of an in-game UI (#417).
//!
//! Unity uGUI's `Canvas` + `CanvasScaler` (Scale With Screen Size) folded into one
//! first-class component. An entity carrying a `CanvasComponent` is a UI root: its
//! descendants carrying a [`RectTransformComponent`](crate::components::RectTransformComponent)
//! are laid out inside the canvas rect by the `ui::layout` system. A screen-space
//! canvas's rect is the whole screen, expressed in *reference units* (see
//! [`CanvasComponent::scale_factor`]); a world-space canvas's is its reference
//! resolution, placed in the scene by its Transform (#429).
//!
//! Authoring data — every field serde-persists except the runtime sway state the
//! `ScreenSpaceCamera` lag writes each tick; the computed rects live in the
//! `ui::UiLayout` resource, never here. The UI model is recorded in `docs/ui.md`.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// Where a canvas draws (Unity's `RenderMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasRenderMode {
    /// Drawn on top of the finished frame, in screen space — HUDs and menus.
    #[default]
    ScreenSpaceOverlay,
    /// Laid out like the overlay, but drawn as a plane `plane_distance` in front of
    /// the active camera — in the scene, so it can tilt and sway with the view (a
    /// visor HUD). Without a camera it behaves as the overlay.
    ScreenSpaceCamera,
    /// A quad in the scene, placed by the canvas's Transform: its rect is the
    /// reference resolution, `pixels_per_unit` reference units to the metre, centred
    /// on the Transform and facing its +Z. Depth-tested, so walls occlude it.
    WorldSpace,
}

/// Unity's default world-canvas density: 100 reference units to the metre.
pub const DEFAULT_PIXELS_PER_UNIT: f32 = 100.0;

/// The `ScreenSpaceCamera` sway's runtime state — never saved (#429). Written by
/// the sway system each tick, read by the renderer and the hit-test.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasSway {
    /// How far the canvas lags behind the view, degrees `(yaw, pitch)`, before
    /// `sway` scales it.
    pub offset: Vec2,
    /// The camera's `(yaw, pitch)` last tick; `None` before the first.
    pub last_look: Option<Vec2>,
}

/// Unity's default *Scale With Screen Size* reference resolution.
pub const DEFAULT_REFERENCE_RESOLUTION: Vec2 = Vec2::new(1920.0, 1080.0);

/// A UI root (Unity's `Canvas` + `CanvasScaler`). See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasComponent {
    /// Where the canvas draws.
    pub render_mode: CanvasRenderMode,
    /// Draw and hit-test order across canvases: higher draws on top and is hit first.
    /// Ties keep scene insertion order.
    pub sort_order: i32,
    /// The resolution the UI is authored at. Layout runs in these units; the scale
    /// factor maps them onto the live screen. Each axis is kept ≥ 1.
    pub reference_resolution: Vec2,
    /// 0 = scale to match the screen's width, 1 = its height, in between = a
    /// log-space blend (Unity's *Match Width Or Height*). Kept in `[0, 1]`.
    pub match_width_or_height: f32,
    /// `WorldSpace`: reference units per metre (kept > 0) — a 400-unit-wide canvas
    /// at the default 100 is 4 m wide.
    pub pixels_per_unit: f32,
    /// `ScreenSpaceCamera`: how far in front of the camera the plane sits, in metres
    /// (kept ≥ 0.01). At zero tilt the canvas exactly fills the view there.
    pub plane_distance: f32,
    /// `ScreenSpaceCamera`: the plane's tilt in degrees, `(x, y)` — about its
    /// horizontal axis (top away from the viewer when positive) and its vertical
    /// axis (right edge away when positive).
    pub tilt: Vec2,
    /// `ScreenSpaceCamera`: how much the canvas lags behind the view when the
    /// camera turns, `0` (glued) … `1` (the whole turn), recovering over
    /// [`SWAY_RECOVERY`](crate::ui::SWAY_RECOVERY) seconds. Kept in `[0, 1]`.
    pub sway: f32,
    /// Runtime only, never saved: the sway lag state.
    #[serde(skip)]
    pub sway_state: CanvasSway,
}

impl Default for CanvasComponent {
    fn default() -> Self {
        Self {
            render_mode: CanvasRenderMode::ScreenSpaceOverlay,
            sort_order: 0,
            reference_resolution: DEFAULT_REFERENCE_RESOLUTION,
            match_width_or_height: 0.0,
            pixels_per_unit: DEFAULT_PIXELS_PER_UNIT,
            plane_distance: 1.0,
            tilt: Vec2::ZERO,
            sway: 0.0,
            sway_state: CanvasSway::default(),
        }
    }
}

impl CanvasComponent {
    /// Screen pixels per reference unit on a `screen`-pixel screen — Unity's
    /// *Scale With Screen Size* in `Match Width Or Height` mode: the width and height
    /// ratios are blended in log2 space, so a 0.5 match on a screen twice as wide and
    /// equally tall scales by √2 rather than 1.5.
    /// A `WorldSpace` canvas has no screen: its scale factor is 1.
    pub fn scale_factor(&self, screen: Vec2) -> f32 {
        if self.is_world_space() {
            return 1.0;
        }
        let reference = self.reference_resolution.max(Vec2::ONE);
        let screen = screen.max(Vec2::ONE);
        let log_w = (screen.x / reference.x).log2();
        let log_h = (screen.y / reference.y).log2();
        let t = self.match_width_or_height.clamp(0.0, 1.0);
        (log_w + (log_h - log_w) * t).exp2()
    }

    /// The canvas rect's size in reference units on a `screen`-pixel screen: the
    /// screen divided by the scale factor. Equals the reference resolution only when
    /// the screen's aspect matches it (or the match favours the axis that differs).
    /// A `WorldSpace` canvas's rect is its reference resolution, whatever the screen.
    pub fn size(&self, screen: Vec2) -> Vec2 {
        if self.is_world_space() {
            return self.reference_resolution.max(Vec2::ONE);
        }
        screen.max(Vec2::ONE) / self.scale_factor(screen)
    }

    /// Whether the canvas is a quad in the scene (`WorldSpace`).
    pub fn is_world_space(&self) -> bool {
        self.render_mode == CanvasRenderMode::WorldSpace
    }
}
