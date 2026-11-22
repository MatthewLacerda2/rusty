use super::fxaa::fxaa_enabled;
use super::motion_blur::effect::blur_params;
use super::ssr::{march, ssr_enabled};
use super::{composite_layout, composite_slots, EffectCx};
use crate::postfx::params::borrow_post_source;
use rusty_components::{CameraComponent, PostProcessingComponent, Tonemap};
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_core::settings::quality::{EffectQuality, QualityPresets};
use rusty_core::settings::GraphicsSettings;
use rusty_scene::Scene;

mod lens;
mod taa;

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

fn slots_of(cx: &EffectCx, effect: &str) -> Vec<[f32; 4]> {
    let all = composite_slots(cx);
    composite_layout()
        .iter()
        .filter(|p| p.effect.name == effect)
        .flat_map(|p| all[p.offset..p.offset + p.function.slots].to_vec())
        .collect()
}

fn blur(scene: &Scene, camera: Option<u32>, graphics: GraphicsSettings) -> Option<f32> {
    let post = borrow_post_source(scene, camera);
    blur_params(post.source(), graphics, 100).map(|b| b.shape[2])
}

fn ssr(scene: &Scene, camera: Option<u32>, graphics: GraphicsSettings) -> Option<u32> {
    let guards = borrow_post_source(scene, camera);
    let post = guards.source();
    let vc = post.volume.filter(|_| ssr_enabled(post, graphics, true))?;
    Some(march(&vc.ssr, graphics).steps)
}

fn fxaa(scene: &Scene, camera: Option<u32>) -> bool {
    fxaa_enabled(
        borrow_post_source(scene, camera).source(),
        Default::default(),
    )
}

fn vc() -> PostProcessingComponent {
    let mut vc = rusty_scene::create::default_post_processing();
    (vc.bloom.intensity, vc.bloom.threshold) = (2.0, 0.7);
    vc.exposure.fixed_ev = 1.5;
    vc.ssr.quality = EffectQuality::Low;
    vc.tonemapping.mode = Tonemap::Reinhard;
    vc
}

fn cam() -> CameraComponent {
    CameraComponent {
        far: 100.0,
        motion_blur_active: true,
        motion_blur_quality: EffectQuality::Custom,
        motion_blur_samples: 16,
        ..CameraComponent::default()
    }
}

fn high() -> GraphicsSettings {
    Default::default()
}

#[test]
fn the_volumes_knobs_reach_their_effects_slots() {
    let (scene, id) = scene_with_vc(vc(), None);
    let medium = Default::default();
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
fn every_preset_runs_ssr_and_motion_blur() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    for preset in &QualityPresets::default().presets {
        let graphics = GraphicsSettings {
            quality: preset.values,
            ..high()
        };
        let steps = preset.values.ssr.low.steps;
        assert_eq!(
            ssr(&scene, Some(id), graphics),
            Some(steps),
            "{}",
            preset.name
        );
        let samples = blur(&scene, Some(id), graphics);
        assert_eq!(samples, Some(16.0), "motion-blur samples passed through");
    }
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
    assert_eq!(ev(bare), 0.0);
    assert!(fxaa(&scene, None), "no camera: FXAA stays default-on");
}

fn forbidding(option: GraphicsOption) -> GraphicsSettings {
    let mut options = GraphicsOptions::default();
    options.set(option, false);
    GraphicsSettings {
        options,
        ..Default::default()
    }
}

#[test]
fn a_player_option_off_gates_the_cameras_effect() {
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let off = forbidding(GraphicsOption::MotionBlur);
    assert_eq!(blur(&scene, Some(id), off), None, "motion blur off");
    assert!(ssr(&scene, Some(id), off).is_some(), "SSR untouched");
    assert!(blur(&scene, Some(id), high()).is_some(), "allowed: it runs");
    let bloom = |g| packed(&scene, Some(id), g, "bloom")[0][3];
    assert_eq!(
        (bloom(high()), bloom(forbidding(GraphicsOption::Bloom))),
        (1.0, 0.0)
    );
    let forbidden = ssr(&scene, Some(id), forbidding(GraphicsOption::Ssr));
    assert_eq!(forbidden, None, "ssr off");
    let source = borrow_post_source(&scene, Some(id));
    assert!(!fxaa_enabled(
        source.source(),
        forbidding(GraphicsOption::Fxaa)
    ));
}

#[test]
fn motion_blur_needs_no_volume() {
    let mut scene = Scene::new();
    let id = add(&mut scene, None, Some(cam()));
    assert_eq!(blur(&scene, Some(id), high()), Some(16.0));
}
