//! `build_post_params` and friends: one view's camera settings into `PostParams`.

use super::*;
use crate::components::{CameraComponent, Tonemap, VisualCorrectionComponent};
use crate::scene::Scene;

/// One entity carrying `vc` (and `cam`), and its id.
fn scene_with_vc(vc: VisualCorrectionComponent, cam: Option<CameraComponent>) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = add(&mut scene, Some(vc), cam);
    (scene, id)
}

fn add(
    scene: &mut Scene,
    vc: Option<VisualCorrectionComponent>,
    cam: Option<CameraComponent>,
) -> u32 {
    let id = scene.add_entity("PostFx".to_string());
    scene.world.set_visual_correction(id, vc);
    scene.world.set_camera(id, cam);
    id
}

fn params(scene: &Scene, camera: Option<u32>, quality: QualityPreset) -> (PostParams, bool) {
    let id = Mat4::IDENTITY;
    build_post_params(scene, camera, quality, id, id, [1.0; 3])
}

fn vc() -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        bloom_intensity: 2.0,
        bloom_threshold: 0.7,
        exposure: 1.5,
        contrast: 1.2,
        saturation: 0.8,
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::Reinhard,
        gamma: 1.3,
        ..crate::scene::authoring::default_visual_correction()
    }
}

fn cam() -> CameraComponent {
    CameraComponent {
        far: 100.0,
        motion_blur_active: true,
        motion_blur_samples: 16,
        ..CameraComponent::default()
    }
}

#[test]
fn color_knobs_flow_into_params() {
    let (scene, id) = scene_with_vc(vc(), None);
    let (p, bloom) = params(&scene, Some(id), QualityPreset::Medium);
    assert_eq!(p.color, [1.5, 1.2, 0.8, 1.3]);
    assert_eq!(p.bloom[0], 2.0); // intensity
    assert_eq!(p.bloom[1], 0.7); // threshold
    assert_eq!(p.bloom[2], Tonemap::Reinhard.to_index() as f32);
    assert_eq!(p.bloom[3], 1.0); // bloom enabled
    assert!(bloom);
}

#[test]
fn game_time_rides_in_camera_pos_w() {
    let (mut scene, id) = scene_with_vc(vc(), None);
    scene.shader_time = 2.5;
    let (p, _) = params(&scene, Some(id), QualityPreset::Medium);
    assert_eq!(p.camera_pos, [1.0, 1.0, 1.0, 2.5]);
}

#[test]
fn low_preset_disables_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::Low);
    assert_eq!(p.flags[0], 0.0, "ssr off on Low");
    assert_eq!(p.flags[1], 0.0, "motion blur off on Low");
    assert_eq!(p.misc[3], 0.0, "no motion-blur samples on Low");
}

#[test]
fn high_preset_enables_real_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::High);
    assert_eq!(p.flags[0], 2.0, "real screen-space SSR on High");
    assert_eq!(p.flags[1], 1.0, "motion blur on on High");
    assert_eq!(p.misc[3], 16.0, "motion-blur samples passed through");
}

#[test]
fn custom_effects_come_from_the_cameras_active_volume_only() {
    let mut v = vc();
    v.custom_effects = vec!["crt".into(), "grain".into()];
    let (scene, id) = scene_with_vc(v.clone(), None);
    assert_eq!(
        post_passes(&scene, Some(id), false).custom,
        ["crt", "grain"]
    );
    assert!(post_passes(&scene, None, false).custom.is_empty());
    v.active = false;
    let (scene, id) = scene_with_vc(v, None);
    assert!(post_passes(&scene, Some(id), false).custom.is_empty());
}

#[test]
fn inactive_volume_and_no_camera_are_neutral_passthrough() {
    let mut v = vc();
    v.active = false;
    let (scene, id) = scene_with_vc(v, None);
    for camera in [Some(id), None] {
        let (p, bloom) = params(&scene, camera, QualityPreset::High);
        // Defaults: exposure 0, contrast/sat 1, gamma 1, no tonemap/bloom.
        assert_eq!(p.color, [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(p.bloom[3], 0.0);
        assert!(!bloom);
    }
}

/// #1434: each view reads its own camera, never the scene's first volume or camera.
#[test]
fn each_view_reads_its_own_camera_whatever_the_entity_order() {
    let mut scene = Scene::new();
    let mut grey = vc();
    grey.saturation = 0.0;
    let no_fxaa = CameraComponent {
        fxaa_active: false,
        ..cam()
    };
    let first = add(&mut scene, Some(vc()), Some(cam()));
    let main = add(&mut scene, Some(grey), Some(no_fxaa));
    let bare = add(&mut scene, None, Some(cam()));

    assert_eq!(
        params(&scene, Some(main), QualityPreset::High).0.color[2],
        0.0
    );
    assert_eq!(
        params(&scene, Some(first), QualityPreset::High).0.color[2],
        0.8
    );
    assert!(!fxaa_enabled(&scene, Some(main)));
    assert!(fxaa_enabled(&scene, Some(first)));
    // A camera with no volume is neutral even though others have one.
    assert_eq!(
        params(&scene, Some(bare), QualityPreset::High).0.color,
        [0.0, 1.0, 1.0, 1.0]
    );
    assert!(
        fxaa_enabled(&scene, None),
        "no camera: FXAA stays default-on"
    );
}
