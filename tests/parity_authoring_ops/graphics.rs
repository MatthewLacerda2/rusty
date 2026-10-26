//! `Graphics.*` bindings vs `authoring::{rendering::post_processing, rendering::camera}`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, ExposureMode, PostProcessingComponent, Tonemap, Wheel};
use rusty::core::settings::quality::QualityPreset;
use rusty::core::settings::GraphicsSettings;
use rusty::scene::authoring::{
    rendering::camera as camera_ops, rendering::post_processing as vc_ops,
};
use rusty::scene::Scene;

/// A scene with one active post-FX volume + active camera; returns `(scene_rc, id)`.
fn scene_with_volume_and_cam() -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("PostFx".to_string());
    let mut vc: PostProcessingComponent = rusty::scene::authoring::default_post_processing();
    (vc.bloom.active, vc.bloom.threshold) = (false, 1.0);
    (vc.ssr.active, vc.ssr.temporal_upsampling) = (false, false);
    vc.ssr.quality = "Low".to_string();
    vc.tonemapping.display_gamma = 2.2;
    scene.world.set_post_processing(id, Some(vc));
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            fov: 60.0,
            far: 100.0,
            motion_blur_samples: 8,
            ..CameraComponent::default()
        }),
    );
    (Rc::new(RefCell::new(scene)), id)
}

/// Apply the same writes the `Graphics.*` script does, via the shared ops directly.
fn apply_graphics_ops(scene: &Rc<RefCell<Scene>>, id: u32) {
    let mut sc = scene.borrow_mut();
    {
        let mut vc = sc.world.post_processing_mut(id).unwrap();
        vc_ops::bloom::set_active(&mut vc, true);
        vc_ops::bloom::set_intensity(&mut vc, -3.0); // clamped to 0
        vc_ops::grading::tonemapping::set_display_gamma(&mut vc, -5.0); // clamped to 0.01
        vc_ops::grading::tonemapping::set_mode(&mut vc, Tonemap::Reinhard);
        vc_ops::grading::white_balance::set(&mut vc, 40.0, 300.0); // tint clamped to 100
        vc_ops::grading::lift_gamma_gain::set_wheel(&mut vc, Wheel::Gain, [0.5, 1.0, 1.0], 0.2);
        vc_ops::grading::color_adjustments::set_hue_shift(&mut vc, 45.0);
        vc_ops::exposure::set_mode(&mut vc, ExposureMode::Automatic);
        vc_ops::exposure::set_limits(&mut vc, -2.0, 30.0); // max clamped to 10
        vc_ops::exposure::set_adaptation_speed(&mut vc, 2.0, -1.0); // down clamped to 0
        vc_ops::grading::color_lookup::set_lut(&mut vc, Some("assets/warm.cube".into()));
        vc_ops::grading::color_lookup::set_contribution(&mut vc, 7.0); // clamped to 1
        vc_ops::ssr::set_active(&mut vc, true);
        vc_ops::ssr::set_quality(&mut vc, "High".to_string());
        vc_ops::volume::set_shadow_cascades(&mut vc, 9); // clamped to 4
        vc_ops::volume::set_shadow_distance(&mut vc, 60.0);
        vc_ops::volume::set_ssao_radius(&mut vc, -1.0); // clamped to 0.01
        vc_ops::volume::set_ssao_intensity(&mut vc, 9.0); // clamped to 4
        vc_ops::volume::set_ssao_active(&mut vc, false);
        vc_ops::custom_effects::set_names(&mut vc, vec!["crt".into(), "hurt".into()]).unwrap();
    }
    let mut cam = sc.world.camera_mut(id).unwrap();
    camera_ops::set_motion_blur_active(&mut cam, true);
    camera_ops::set_motion_blur_samples(&mut cam, 16);
    camera_ops::set_motion_blur_intensity(&mut cam, 2.0); // clamped to 1
    camera_ops::set_motion_blur_max_length(&mut cam, 0.1);
    camera_ops::set_fxaa_active(&mut cam, false);
}

/// The `Graphics.*` writes the parity test drives, mirrored by `apply_graphics_ops`.
const GRAPHICS_SCRIPT: &str = r#"
Graphics.SetBloomActive(true)
Graphics.SetBloomIntensity(-3.0)
Graphics.SetGamma(-5.0)
Graphics.SetTonemap("reinhard")
Graphics.SetWhiteBalance(40, 300)
Graphics.SetGainWheel(0.5, 1, 1, 0.2)
Graphics.SetHueShift(45)
Graphics.SetExposureMode("Automatic")
Graphics.SetExposureLimits(-2, 30)
Graphics.SetAdaptationSpeed(2, -1)
Graphics.SetColorLookup("assets/warm.cube")
Graphics.SetColorLookupContribution(7)
Graphics.SetSsrActive(true)
Graphics.SetSsrQuality("High")
Graphics.SetShadowCascades(9)
Graphics.SetShadowDistance(60)
Graphics.SetSsaoRadius(-1)
Graphics.SetSsaoIntensity(9)
Graphics.SetSsaoActive(false)
assert(Graphics.GetSsaoIntensity() == 4 and not Graphics.GetSsaoActive())
Graphics.SetCustomEffects({"crt", "hurt"})
assert(not pcall(Graphics.SetCustomEffects, {"../x"}) and #Graphics.GetCustomEffects() == 2)
Graphics.SetMotionBlurActive(true)
Graphics.SetMotionBlurSamples(16)
Graphics.SetMotionBlurIntensity(2.0)
Graphics.SetMotionBlurMaxLength(0.1)
Graphics.SetFxaaActive(false)
"#;

#[test]
fn graphics_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // `Graphics.*` drives the active entity's post-processing + camera-motion-blur
    // through the SAME ops the editor cards call. We run the bindings against one
    // scene and the ops against a second cloned scene, then compare the components.
    let (lua_scene, lua_id) = scene_with_volume_and_cam();
    let (op_scene, op_id) = scene_with_volume_and_cam();
    let quality = Rc::new(RefCell::new(GraphicsSettings::from(QualityPreset::High)));
    let app = RefCell::default();

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &quality, &app).unwrap();
        lua.load(GRAPHICS_SCRIPT).exec().unwrap();
        Ok(())
    })?;

    apply_graphics_ops(&op_scene, op_id);

    let ls = lua_scene.borrow();
    let os = op_scene.borrow();
    let lvc = ls.world.post_processing(lua_id).unwrap().clone();
    let ovc = os.world.post_processing(op_id).unwrap().clone();
    // Every field, through `Debug`: the two paths must leave identical components.
    assert_eq!(format!("{lvc:?}"), format!("{ovc:?}"));
    assert_eq!(lvc.bloom.intensity, 0.0, "single-sourced clamp");
    assert_eq!(lvc.tonemapping.display_gamma, 0.01, "single-sourced clamp");
    let lcam = ls.world.camera(lua_id).unwrap().clone();
    let ocam = os.world.camera(op_id).unwrap().clone();
    assert_eq!(format!("{lcam:?}"), format!("{ocam:?}"));
    assert!(
        !lcam.fxaa_active,
        "both paths turned the default-on knob off"
    );
    Ok(())
}
