//! What each effect packs for one view's camera (#1434, #1467): its composite slots
//! from the volume and the camera, gated by the tier and the player's options.

use super::fxaa::fxaa_enabled;
use super::motion_blur::effect::blur_params;
use super::{composite_layout, composite_slots, EffectCx};
use crate::components::{CameraComponent, PostProcessingComponent, Tonemap};
use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::borrow_post_source;
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

/// `effect`'s composite slots for `camera`'s view under `graphics`.
fn packed(
    scene: &Scene,
    camera: Option<u32>,
    graphics: GraphicsSettings,
    effect: &str,
) -> Vec<[f32; 4]> {
    let guards = borrow_post_source(scene, camera);
    let cx = EffectCx {
        post: guards.source(),
        graphics,
        full: true,
    };
    slots_of(&cx, effect)
}

/// `effect`'s composite slots under `cx`.
fn slots_of(cx: &EffectCx, effect: &str) -> Vec<[f32; 4]> {
    let all = composite_slots(cx);
    composite_layout()
        .iter()
        .filter(|p| p.effect.name == effect)
        .flat_map(|p| all[p.offset..p.offset + p.function.slots].to_vec())
        .collect()
}

/// The motion blur `camera` runs under `graphics`, as its sample count.
fn blur(scene: &Scene, camera: Option<u32>, graphics: GraphicsSettings) -> Option<f32> {
    let post = borrow_post_source(scene, camera);
    blur_params(post.source(), graphics, 100).map(|b| b.shape[2])
}

fn fxaa(scene: &Scene, camera: Option<u32>) -> bool {
    fxaa_enabled(
        borrow_post_source(scene, camera).source(),
        Default::default(),
    )
}

fn vc() -> PostProcessingComponent {
    let mut vc = crate::scene::authoring::default_post_processing();
    (vc.bloom.intensity, vc.bloom.threshold) = (2.0, 0.7);
    vc.exposure.fixed_ev = 1.5;
    vc.ssr.temporal_upsampling = false;
    vc.tonemapping.mode = Tonemap::Reinhard;
    vc
}

fn cam() -> CameraComponent {
    CameraComponent {
        far: 100.0,
        motion_blur_active: true,
        motion_blur_samples: 16,
        ..CameraComponent::default()
    }
}

/// The High tier, every option allowed.
fn high() -> GraphicsSettings {
    QualityPreset::High.into()
}

#[test]
fn the_volumes_knobs_reach_their_effects_slots() {
    let (scene, id) = scene_with_vc(vc(), None);
    let medium = QualityPreset::Medium.into();
    assert_eq!(
        packed(&scene, Some(id), medium, "exposure"),
        [[1.5, 0.0, 0.0, 0.0]]
    );
    assert_eq!(
        packed(&scene, Some(id), medium, "bloom"),
        [[2.0, 0.0, 0.0, 1.0]]
    );
}

#[test]
fn low_preset_disables_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let low = QualityPreset::Low.into();
    assert_eq!(
        packed(&scene, Some(id), low, "ssr")[0][0],
        0.0,
        "ssr off on Low"
    );
    assert_eq!(blur(&scene, Some(id), low), None);
}

#[test]
fn high_preset_enables_real_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let ssr = packed(&scene, Some(id), high(), "ssr")[0][0];
    assert_eq!(ssr, 2.0, "real screen-space SSR on High");
    let samples = blur(&scene, Some(id), high());
    assert_eq!(samples, Some(16.0), "motion-blur samples passed through");
}

#[test]
fn inactive_volume_no_camera_and_a_display_only_view_are_neutral() {
    let mut v = vc();
    v.active = false;
    let (scene, id) = scene_with_vc(v, None);
    let neutral = composite_slots(&EffectCx {
        post: Default::default(),
        graphics: high(),
        full: true,
    });
    for camera in [Some(id), None] {
        let guards = borrow_post_source(&scene, camera);
        let cx = EffectCx {
            post: guards.source(),
            graphics: high(),
            full: true,
        };
        assert_eq!(composite_slots(&cx), neutral);
    }
    assert!(
        neutral.iter().flatten().all(|&x| x == 0.0),
        "every effect skips"
    );
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let guards = borrow_post_source(&scene, Some(id));
    let display_only = EffectCx {
        post: guards.source(),
        graphics: high(),
        full: false,
    };
    assert_eq!(composite_slots(&display_only), neutral);
}

/// #1434: each view reads its own camera, never the scene's first volume or camera.
#[test]
fn each_view_reads_its_own_camera_whatever_the_entity_order() {
    let mut scene = Scene::new();
    let mut dark = vc();
    dark.exposure.fixed_ev = -2.0;
    let no_fxaa = CameraComponent {
        fxaa_active: false,
        ..cam()
    };
    let first = add(&mut scene, Some(vc()), Some(cam()));
    let main = add(&mut scene, Some(dark), Some(no_fxaa));
    let bare = add(&mut scene, None, Some(cam()));
    let ev = |id| packed(&scene, Some(id), high(), "exposure")[0][0];
    assert_eq!((ev(main), ev(first)), (-2.0, 1.5));
    assert!(!fxaa(&scene, Some(main)));
    assert!(fxaa(&scene, Some(first)));
    // A camera with no volume is neutral even though others have one.
    assert_eq!(ev(bare), 0.0);
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
    let off = forbidding(GraphicsOption::MotionBlur);
    assert_eq!(blur(&scene, Some(id), off), None, "motion blur off");
    assert_eq!(
        packed(&scene, Some(id), off, "ssr")[0][0],
        2.0,
        "SSR untouched"
    );
    assert!(blur(&scene, Some(id), high()).is_some(), "allowed: it runs");
    let bloom = |g| packed(&scene, Some(id), g, "bloom")[0][3];
    assert_eq!(
        (bloom(high()), bloom(forbidding(GraphicsOption::Bloom))),
        (1.0, 0.0)
    );
    let ssr = packed(&scene, Some(id), forbidding(GraphicsOption::Ssr), "ssr");
    assert_eq!(ssr[0][0], 0.0, "ssr off");
    let source = borrow_post_source(&scene, Some(id));
    assert!(!fxaa_enabled(
        source.source(),
        forbidding(GraphicsOption::Fxaa)
    ));
}

/// #1436: motion blur is the camera's, like FXAA: it runs without a volume.
#[test]
fn motion_blur_needs_no_volume() {
    let mut scene = Scene::new();
    let id = add(&mut scene, None, Some(cam()));
    assert_eq!(blur(&scene, Some(id), high()), Some(16.0));
}
