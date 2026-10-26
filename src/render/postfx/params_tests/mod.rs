//! `build_post_params` and friends: one view's camera settings into `PostParams`.

use super::*;
use crate::components::{CameraComponent, PostProcessingComponent, Tonemap};
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::core::settings::quality::QualityPreset;
use crate::scene::Scene;

mod lens;

/// One entity carrying `vc` (and `cam`), and its id.
fn scene_with_vc(vc: PostProcessingComponent, cam: Option<CameraComponent>) -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = add(&mut scene, Some(vc), cam);
    (scene, id)
}

fn add(
    scene: &mut Scene,
    vc: Option<PostProcessingComponent>,
    cam: Option<CameraComponent>,
) -> u32 {
    let id = scene.add_entity("PostFx".to_string());
    scene.world.set_post_processing(id, vc);
    scene.world.set_camera(id, cam);
    id
}

fn params(scene: &Scene, camera: Option<u32>, quality: QualityPreset) -> (PostParams, bool) {
    let post = borrow_post_source(scene, camera);
    build_post_params(
        scene,
        post.source(),
        quality.into(),
        Mat4::IDENTITY,
        [1.0; 3],
    )
}

/// The motion blur `camera` runs under `graphics`, as its sample count.
fn blur(scene: &Scene, camera: Option<u32>, graphics: GraphicsSettings) -> Option<f32> {
    let post = borrow_post_source(scene, camera);
    blur_params(post.source(), graphics, 100).map(|b| b.shape[2])
}

fn custom(scene: &Scene, camera: Option<u32>) -> Vec<String> {
    post_passes(
        borrow_post_source(scene, camera).source(),
        false,
        Default::default(),
    )
    .custom
    .to_vec()
}

fn fxaa(scene: &Scene, camera: Option<u32>) -> bool {
    fxaa_enabled(
        borrow_post_source(scene, camera).source(),
        Default::default(),
    )
}

fn vc() -> PostProcessingComponent {
    PostProcessingComponent {
        bloom_intensity: 2.0,
        bloom_threshold: 0.7,
        exposure: 1.5,
        contrast: 1.2,
        saturation: 0.8,
        ssr_temporal_upsampling: false,
        tonemap: Tonemap::Reinhard,
        gamma: 1.3,
        ..crate::scene::authoring::default_post_processing()
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
    assert_eq!(blur(&scene, Some(id), QualityPreset::Low.into()), None);
}

#[test]
fn high_preset_enables_real_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::High);
    assert_eq!(p.flags[0], 2.0, "real screen-space SSR on High");
    let samples = blur(&scene, Some(id), QualityPreset::High.into());
    assert_eq!(samples, Some(16.0), "motion-blur samples passed through");
}

#[test]
fn custom_effects_come_from_the_cameras_active_volume_only() {
    let mut v = vc();
    v.custom_effects = vec!["crt".into(), "grain".into()];
    let (scene, id) = scene_with_vc(v.clone(), None);
    assert_eq!(custom(&scene, Some(id)), ["crt", "grain"]);
    assert!(custom(&scene, None).is_empty());
    v.active = false;
    let (scene, id) = scene_with_vc(v, None);
    assert!(custom(&scene, Some(id)).is_empty());
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
    assert!(!fxaa(&scene, Some(main)));
    assert!(fxaa(&scene, Some(first)));
    // A camera with no volume is neutral even though others have one.
    assert_eq!(
        params(&scene, Some(bare), QualityPreset::High).0.color,
        [0.0, 1.0, 1.0, 1.0]
    );
    assert!(fxaa(&scene, None), "no camera: FXAA stays default-on");
}

/// High tier with the player's `option` forbidden.
fn forbidding(option: GraphicsOption) -> GraphicsSettings {
    let mut options = GraphicsOptions::default();
    options.set(option, false);
    GraphicsSettings {
        quality: QualityPreset::High,
        options,
    }
}

/// #1438: a player option off turns its effect off for a camera that has it on;
/// allowed, the camera's choice stands.
#[test]
fn a_player_option_off_gates_the_cameras_effect() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let run = |g: GraphicsSettings| {
        let post = borrow_post_source(&scene, Some(id));
        build_post_params(&scene, post.source(), g, Mat4::IDENTITY, [1.0; 3])
    };
    let off = forbidding(GraphicsOption::MotionBlur);
    assert_eq!(blur(&scene, Some(id), off), None, "motion blur off");
    assert_eq!(run(off).0.flags[0], 2.0, "SSR untouched");
    let on = blur(&scene, Some(id), QualityPreset::High.into());
    assert!(on.is_some(), "allowed: the camera's motion blur runs");
    assert!(run(QualityPreset::High.into()).1);
    let (p, bloom) = run(forbidding(GraphicsOption::Bloom));
    assert!(!bloom && p.bloom[3] == 0.0, "bloom off");
    assert_eq!(
        run(forbidding(GraphicsOption::Ssr)).0.flags[0],
        0.0,
        "ssr off"
    );
    let source = borrow_post_source(&scene, Some(id));
    assert!(!fxaa_enabled(
        source.source(),
        forbidding(GraphicsOption::Fxaa)
    ));
}

/// An option allows; it never turns on what the camera or the tier left off.
#[test]
fn effect_runs_needs_camera_option_and_tier() {
    let all = GraphicsSettings::from(QualityPreset::High);
    assert!(effect_runs(all, GraphicsOption::Ssr, true));
    assert!(!effect_runs(all, GraphicsOption::Ssr, false), "camera off");
    let medium = GraphicsSettings::from(QualityPreset::Medium);
    assert!(!effect_runs(medium, GraphicsOption::Ssr, true), "tier off");
    let off = forbidding(GraphicsOption::Ssr);
    assert!(!effect_runs(off, GraphicsOption::Ssr, true), "option off");
}

/// #1436: motion blur is the camera's, like FXAA: it runs without a volume.
#[test]
fn motion_blur_needs_no_volume() {
    let mut scene = Scene::new();
    let id = add(&mut scene, None, Some(cam()));
    assert_eq!(
        blur(&scene, Some(id), QualityPreset::High.into()),
        Some(16.0)
    );
}
