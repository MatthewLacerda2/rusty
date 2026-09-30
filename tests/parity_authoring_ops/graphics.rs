//! `Graphics.*` bindings vs `authoring::{visual_correction, camera}`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, ClearFlags, Tonemap, VisualCorrectionComponent};
use rusty::core::quality::QualityPreset;
use rusty::scene::authoring::{camera as camera_ops, visual_correction as vc_ops};
use rusty::scene::Scene;

/// A scene with one active post-FX volume + active camera; returns `(scene_rc, id)`.
fn scene_with_volume_and_cam() -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("PostFx".to_string());
    scene.world.set_visual_correction(
        id,
        Some(VisualCorrectionComponent {
            active: true,
            bloom_active: false,
            bloom_intensity: 1.0,
            bloom_threshold: 1.0,
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
    (Rc::new(RefCell::new(scene)), id)
}

/// Apply the same writes the `Graphics.*` script does, via the shared ops directly.
fn apply_graphics_ops(scene: &Rc<RefCell<Scene>>, id: u32) {
    let mut sc = scene.borrow_mut();
    {
        let mut vc = sc.world.visual_correction_mut(id).unwrap();
        vc_ops::set_bloom_active(&mut vc, true);
        vc_ops::set_bloom_intensity(&mut vc, -3.0); // clamped to 0
        vc_ops::set_gamma(&mut vc, -5.0); // clamped to 0.01
        vc_ops::set_tonemap(&mut vc, Tonemap::Reinhard);
        vc_ops::set_ssr_active(&mut vc, true);
        vc_ops::set_ssr_quality(&mut vc, "High".to_string());
    }
    let mut cam = sc.world.camera_mut(id).unwrap();
    camera_ops::set_motion_blur_active(&mut cam, true);
    camera_ops::set_motion_blur_samples(&mut cam, 16);
    camera_ops::set_fxaa_active(&mut cam, false);
}

#[test]
fn graphics_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    // `Graphics.*` drives the active entity's visual-correction + camera-motion-blur
    // through the SAME ops the editor cards call. We run the bindings against one
    // scene and the ops against a second cloned scene, then compare the components.
    let (lua_scene, lua_id) = scene_with_volume_and_cam();
    let (op_scene, op_id) = scene_with_volume_and_cam();
    let quality = Rc::new(RefCell::new(QualityPreset::High));

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &quality).unwrap();
        lua.load(
            r#"
            Graphics.SetBloomActive(true)
            Graphics.SetBloomIntensity(-3.0)
            Graphics.SetGamma(-5.0)
            Graphics.SetTonemap("reinhard")
            Graphics.SetSsrActive(true)
            Graphics.SetSsrQuality("High")
            Graphics.SetMotionBlurActive(true)
            Graphics.SetMotionBlurSamples(16)
            Graphics.SetFxaaActive(false)
        "#,
        )
        .exec()
        .unwrap();
        Ok(())
    })?;

    apply_graphics_ops(&op_scene, op_id);

    let ls = lua_scene.borrow();
    let os = op_scene.borrow();
    let lvc = ls.world.visual_correction(lua_id).unwrap().clone();
    let ovc = os.world.visual_correction(op_id).unwrap().clone();
    assert_eq!(lvc.bloom_active, ovc.bloom_active);
    assert_eq!(lvc.bloom_intensity, ovc.bloom_intensity);
    assert_eq!(lvc.gamma, ovc.gamma);
    assert_eq!(lvc.tonemap, ovc.tonemap);
    assert_eq!(lvc.ssr_active, ovc.ssr_active);
    assert_eq!(lvc.ssr_quality, ovc.ssr_quality);
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
