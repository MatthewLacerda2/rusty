//! src/components/visual_correction.rs — Post-process component
//!
//! bloom/exposure/SSR settings. Unity: post-process volume. Moved verbatim from
//! the legacy `core/scene.rs`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualCorrectionComponent {
    pub active: bool,
    pub bloom_active: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub ssr_active: bool,
    pub ssr_quality: String, // "Low", "Medium", "High", "Ultra"
    pub ssr_temporal_upsampling: bool,
}
