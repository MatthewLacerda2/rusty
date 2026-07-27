//! Integration coverage for the `Graphics` scripting namespace (issue #87): the
//! per-volume post-FX getters/setters write the active `VisualCorrectionComponent`
//! / `CameraComponent`, and the global quality preset round-trips through the
//! shared resource cell. These are render-only, one-way writes — they never feed
//! `FixedUpdate` — so the test only asserts the surface, not sim evolution.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, ClearFlags, Tonemap, VisualCorrectionComponent};
use rusty::render::postfx::QualityPreset;
use rusty::scene::Scene;

fn scene_with_volume() -> Rc<RefCell<Scene>> {
    let mut scene = Scene::new();
    let id = scene.add_entity("PostFx".to_string());
    scene.world.set_visual_correction(
        id,
        Some(VisualCorrectionComponent {
            active: true,
            bloom_active: false,
            bloom_intensity: 1.0,
            bloom_threshold: 0.8,
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            ssr_active: false,
            ssr_quality: "Low".to_string(),
            ssr_temporal_upsampling: false,
            tonemap: Tonemap::Aces,
            gamma: 2.2,
        }),
    );
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            active: true,
            fov: 60.0,
            near: 0.1,
            far: 100.0,
            culling_mask: u32::MAX,
            render_order: 0,
            clear_flags: ClearFlags::Skybox,
            motion_blur_active: false,
            motion_blur_samples: 8,
            fxaa_active: true,
        }),
    );
    Rc::new(RefCell::new(scene))
}

#[test]
fn bloom_and_color_round_trip() {
    let lua = Lua::new();
    let scene = scene_with_volume();
    let quality = Rc::new(RefCell::new(QualityPreset::Medium));

    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &quality).unwrap();
        lua.load(
            r#"
            Graphics.SetBloomActive(true)
            Graphics.SetBloomIntensity(3.5)
            Graphics.SetExposure(1.25)
            Graphics.SetGamma(2.0)
            Graphics.SetTonemap("Reinhard")
        "#,
        )
        .exec()
        .unwrap();

        let s = scene.borrow();
        let id = s.entity_ids()[0];
        let vc = s.world.visual_correction(id).unwrap().clone();
        assert!(vc.bloom_active);
        assert_eq!(vc.bloom_intensity, 3.5);
        assert_eq!(vc.exposure, 1.25);
        assert_eq!(vc.gamma, 2.0);
        assert_eq!(vc.tonemap, Tonemap::Reinhard);

        let read: bool = lua.load("return Graphics.GetBloomActive()").eval().unwrap();
        assert!(read);
        let tm: String = lua.load("return Graphics.GetTonemap()").eval().unwrap();
        assert_eq!(tm, "Reinhard");
        Ok(())
    })
    .unwrap();
}

#[test]
fn motion_blur_samples_are_clamped() {
    let lua = Lua::new();
    let scene = scene_with_volume();
    let quality = Rc::new(RefCell::new(QualityPreset::Medium));

    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &quality).unwrap();
        lua.load("Graphics.SetMotionBlurActive(true); Graphics.SetMotionBlurSamples(99)")
            .exec()
            .unwrap();
        let s = scene.borrow();
        let id = s.entity_ids()[0];
        let cam = s.world.camera(id).unwrap().clone();
        assert!(cam.motion_blur_active);
        assert_eq!(cam.motion_blur_samples, 32, "clamped into 2..=32");
        Ok(())
    })
    .unwrap();
}

#[test]
fn quality_preset_round_trips_through_cell() {
    let lua = Lua::new();
    let scene = scene_with_volume();
    let quality = Rc::new(RefCell::new(QualityPreset::Medium));

    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &quality).unwrap();
        lua.load("Graphics.SetQuality(\"High\")").exec().unwrap();
        assert_eq!(*quality.borrow(), QualityPreset::High);
        let name: String = lua.load("return Graphics.GetQuality()").eval().unwrap();
        assert_eq!(name, "High");

        // An unknown tier name is ignored — the current value is kept.
        lua.load("Graphics.SetQuality(\"Ultra\")").exec().unwrap();
        assert_eq!(*quality.borrow(), QualityPreset::High);
        Ok(())
    })
    .unwrap();
}

#[test]
fn getters_return_defaults_with_no_active_volume() {
    let lua = Lua::new();
    let scene = Rc::new(RefCell::new(Scene::new()));
    let quality = Rc::new(RefCell::new(QualityPreset::Low));

    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &quality).unwrap();
        let bloom: bool = lua.load("return Graphics.GetBloomActive()").eval().unwrap();
        assert!(!bloom);
        let gamma: f32 = lua.load("return Graphics.GetGamma()").eval().unwrap();
        assert_eq!(gamma, 2.2);
        Ok(())
    })
    .unwrap();
}
