use glam::Vec2;
use serde::{Deserialize, Serialize};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum CanvasRenderMode {
        #[default]
        ScreenSpaceOverlay = "ScreenSpaceOverlay" | "Overlay",
        ScreenSpaceCamera = "ScreenSpaceCamera" | "Camera",
        WorldSpace = "WorldSpace" | "World",
    }
}

const DEFAULT_PIXELS_PER_UNIT: f32 = 100.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CanvasSway {
    pub offset: Vec2,
    pub last_look: Option<Vec2>,
}

const DEFAULT_REFERENCE_RESOLUTION: Vec2 = Vec2::new(1920.0, 1080.0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasComponent {
    pub render_mode: CanvasRenderMode,
    pub sort_order: i32,
    pub reference_resolution: Vec2,
    pub match_width_or_height: f32,
    pub pixels_per_unit: f32,
    pub plane_distance: f32,
    pub tilt: Vec2,
    pub sway: f32,
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

    pub fn size(&self, screen: Vec2) -> Vec2 {
        if self.is_world_space() {
            return self.reference_resolution.max(Vec2::ONE);
        }
        screen.max(Vec2::ONE) / self.scale_factor(screen)
    }

    pub fn is_world_space(&self) -> bool {
        self.render_mode == CanvasRenderMode::WorldSpace
    }
}

impl crate::FirstClass for CanvasComponent {
    const NAME: &'static str = "Canvas";
}
