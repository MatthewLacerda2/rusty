//! Builds the per-frame `PostParams` from the scene's active
//! `VisualCorrectionComponent` + `CameraComponent`, gated by the quality preset.
//!
//! This is the bridge from the editor's "dead knobs" to the GPU: every field the
//! inspector exposes ends up packed here and read by `postfx.wgsl`.

use glam::Mat4;

use super::postfx::{PostParams, QualityPreset};
use crate::scene::Scene;

/// Scan `scene` for the first active visual-correction volume and matching camera
/// settings, and pack them into a `PostParams` for `postfx.wgsl`. Returns the
/// params plus whether the bloom passes should run this frame.
///
/// `prev_view_proj` is last frame's view-projection (camera motion-blur velocity).
pub fn build_post_params(
    scene: &Scene,
    quality: QualityPreset,
    view_proj: Mat4,
    prev_view_proj: Mat4,
    camera_pos: [f32; 3],
) -> (PostParams, bool) {
    let mut params = PostParams {
        view_proj: view_proj.to_cols_array(),
        inv_view_proj: view_proj.inverse().to_cols_array(),
        prev_view_proj: prev_view_proj.to_cols_array(),
        camera_pos: [camera_pos[0], camera_pos[1], camera_pos[2], 0.0],
        ..Default::default()
    };

    let mut bloom_enabled = false;

    for entity in scene.iter() {
        if !entity.active {
            continue;
        }
        let Some(vc) = &entity.visual_correction else {
            continue;
        };
        if !vc.active {
            continue;
        }

        params.color = [vc.exposure, vc.contrast, vc.saturation, vc.gamma.max(0.01)];
        let bloom_on = vc.bloom_active;
        params.bloom = [
            vc.bloom_intensity,
            vc.bloom_threshold,
            vc.tonemap.to_index() as f32,
            if bloom_on { 1.0 } else { 0.0 },
        ];
        bloom_enabled = bloom_on;

        // SSR mode: 0 off, 2 = real screen-space (High tier only). Low/Medium keep
        // the forward shader's cubemap reflection, so post-FX SSR stays off there.
        let ssr_mode = if vc.ssr_active && quality.screen_space_ssr() {
            2.0
        } else {
            0.0
        };

        // Motion blur is gated by the camera component AND the preset.
        let (mb_active, mb_samples) = match &entity.camera {
            Some(cam) if cam.motion_blur_active && quality.motion_blur() => {
                (1.0, cam.motion_blur_samples.clamp(2, 32) as f32)
            }
            _ => (0.0, 0.0),
        };
        params.flags = [ssr_mode, mb_active, 1.0, 0.0];
        params.misc[3] = mb_samples;
        break;
    }

    (params, bloom_enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{CameraComponent, ClearFlags, Tonemap, VisualCorrectionComponent};
    use crate::scene::Scene;

    fn scene_with_vc(vc: VisualCorrectionComponent, cam: Option<CameraComponent>) -> Scene {
        let mut scene = Scene::new();
        let id = scene.add_entity("PostFx".to_string());
        let mut e = scene.get_entity_mut(id).unwrap();
        e.visual_correction = Some(vc);
        e.camera = cam;
        drop(e);
        scene
    }

    fn vc() -> VisualCorrectionComponent {
        VisualCorrectionComponent {
            active: true,
            bloom_active: true,
            bloom_intensity: 2.0,
            bloom_threshold: 0.7,
            exposure: 1.5,
            contrast: 1.2,
            saturation: 0.8,
            ssr_active: true,
            ssr_quality: "High".to_string(),
            ssr_temporal_upsampling: false,
            tonemap: Tonemap::Reinhard,
            gamma: 2.2,
        }
    }

    fn cam() -> CameraComponent {
        CameraComponent {
            active: true,
            fov: 45.0,
            near: 0.1,
            far: 100.0,
            culling_mask: u32::MAX,
            render_order: 0,
            clear_flags: ClearFlags::Skybox,
            motion_blur_active: true,
            motion_blur_samples: 16,
        }
    }

    #[test]
    fn color_knobs_flow_into_params() {
        let scene = scene_with_vc(vc(), None);
        let (p, bloom) = build_post_params(
            &scene,
            QualityPreset::Medium,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [0.0; 3],
        );
        assert_eq!(p.color, [1.5, 1.2, 0.8, 2.2]);
        assert_eq!(p.bloom[0], 2.0); // intensity
        assert_eq!(p.bloom[1], 0.7); // threshold
        assert_eq!(p.bloom[2], Tonemap::Reinhard.to_index() as f32);
        assert_eq!(p.bloom[3], 1.0); // bloom enabled
        assert!(bloom);
    }

    #[test]
    fn low_preset_disables_ssr_and_motion_blur() {
        let scene = scene_with_vc(vc(), Some(cam()));
        let (p, _) = build_post_params(
            &scene,
            QualityPreset::Low,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [0.0; 3],
        );
        assert_eq!(p.flags[0], 0.0, "ssr off on Low");
        assert_eq!(p.flags[1], 0.0, "motion blur off on Low");
        assert_eq!(p.misc[3], 0.0, "no motion-blur samples on Low");
    }

    #[test]
    fn high_preset_enables_real_ssr_and_motion_blur() {
        let scene = scene_with_vc(vc(), Some(cam()));
        let (p, _) = build_post_params(
            &scene,
            QualityPreset::High,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [0.0; 3],
        );
        assert_eq!(p.flags[0], 2.0, "real screen-space SSR on High");
        assert_eq!(p.flags[1], 1.0, "motion blur on on High");
        assert_eq!(p.misc[3], 16.0, "motion-blur samples passed through");
    }

    #[test]
    fn inactive_volume_is_neutral_passthrough() {
        let mut v = vc();
        v.active = false;
        let scene = scene_with_vc(v, None);
        let (p, bloom) = build_post_params(
            &scene,
            QualityPreset::High,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            [0.0; 3],
        );
        // Defaults: exposure 0, contrast/sat 1, gamma 1, no tonemap/bloom.
        assert_eq!(p.color, [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(p.bloom[3], 0.0);
        assert!(!bloom);
    }
}
