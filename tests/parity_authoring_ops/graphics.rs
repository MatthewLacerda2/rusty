//! `Graphics.*` bindings vs `authoring::{rendering::post_processing, rendering::camera}`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, PostProcessingComponent, Tonemap, Wheel};
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
    scene.world.set_post_processing(
        id,
        Some(PostProcessingComponent {
            bloom_active: false,
            bloom_threshold: 1.0,
            ssr_active: false,
            ssr_quality: "Low".to_string(),
            ssr_temporal_upsampling: false,
            gamma: 2.2,
            ..rusty::scene::authoring::default_post_processing()
        }),
    );
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
        vc_ops::set_bloom_active(&mut vc, true);
        vc_ops::set_bloom_intensity(&mut vc, -3.0); // clamped to 0
        vc_ops::set_gamma(&mut vc, -5.0); // clamped to 0.01
        vc_ops::set_tonemap(&mut vc, Tonemap::Reinhard);
        vc_ops::set_white_balance(&mut vc, 40.0, 300.0); // tint clamped to 100
        vc_ops::set_wheel(&mut vc, Wheel::Gain, [0.5, 1.0, 1.0], 0.2);
        vc_ops::set_hue_shift(&mut vc, 45.0);
        vc_ops::set_ssr_active(&mut vc, true);
        vc_ops::set_ssr_quality(&mut vc, "High".to_string());
        vc_ops::set_shadow_cascades(&mut vc, 9); // clamped to 4
        vc_ops::set_shadow_distance(&mut vc, 60.0);
        vc_ops::set_ssao_radius(&mut vc, -1.0); // clamped to 0.01
        vc_ops::set_ssao_intensity(&mut vc, 9.0); // clamped to 4
        vc_ops::set_ssao_active(&mut vc, false);
        vc_ops::set_custom_effects(&mut vc, vec!["crt".into(), "hurt".into()]).unwrap();
    }
    let mut cam = sc.world.camera_mut(id).unwrap();
    camera_ops::set_motion_blur_active(&mut cam, true);
    camera_ops::set_motion_blur_samples(&mut cam, 16);
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
    assert_eq!(lvc.bloom_active, ovc.bloom_active);
    assert_eq!(lvc.bloom_intensity, ovc.bloom_intensity);
    assert_eq!(lvc.gamma, ovc.gamma);
    assert_eq!(lvc.tonemap, ovc.tonemap);
    assert_eq!(lvc.white_balance, ovc.white_balance);
    assert_eq!(lvc.lift_gamma_gain, ovc.lift_gamma_gain);
    assert_eq!(lvc.hue_shift, ovc.hue_shift);
    assert_eq!(lvc.ssr_active, ovc.ssr_active);
    assert_eq!(lvc.ssr_quality, ovc.ssr_quality);
    assert_eq!(lvc.shadows, ovc.shadows);
    assert_eq!(lvc.ssao, ovc.ssao);
    assert_eq!(lvc.custom_effects, ovc.custom_effects);
    assert_eq!(lvc.bloom_intensity, 0.0, "single-sourced clamp");
    assert_eq!(lvc.gamma, 0.01, "single-sourced clamp");
    let lcam = ls.world.camera(lua_id).unwrap().clone();
    let ocam = os.world.camera(op_id).unwrap().clone();
    assert_eq!(lcam.motion_blur_active, ocam.motion_blur_active);
    assert_eq!(lcam.motion_blur_samples, ocam.motion_blur_samples);
    assert_eq!(lcam.fxaa_active, ocam.fxaa_active);
    assert!(
        !lcam.fxaa_active,
        "both paths turned the default-on knob off"
    );
    Ok(())
}
