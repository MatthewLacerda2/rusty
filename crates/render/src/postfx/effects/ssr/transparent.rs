use super::{fade, march};
use crate::postfx::params::PostSource;
use rusty_components::MaterialAsset;
use rusty_core::settings::GraphicsSettings;

pub(crate) fn camera_march(post: PostSource, graphics: GraphicsSettings, on: bool) -> [f32; 4] {
    match post.volume.filter(|_| on) {
        Some(vc) => {
            let m = march(&vc.ssr, graphics);
            let [floor, width] = fade(vc.ssr.min_smoothness.max(SHARP_SMOOTHNESS));
            [m.steps as f32, m.step_length, floor, width]
        }
        None => [0.0; 4],
    }
}

// The copy has no mip pyramid, so a translucent's hit is sharp: never reflect below this.
pub(crate) const SHARP_SMOOTHNESS: f32 = 0.7;

/// Reads the authored roughness: specular AA only widens it, so a material refused here could
/// never reflect.
pub(crate) fn traces(m: &MaterialAsset) -> bool {
    let smooth = 1.0 - m.roughness.max(0.04) - SHARP_SMOOTHNESS > 0.0;
    m.receives_ssr() && (smooth || m.roughness_map.is_some())
}
