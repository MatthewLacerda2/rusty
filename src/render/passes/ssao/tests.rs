//! The SSAO plan (#436): when the passes run, and what the shader is told.

use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

use super::{SsaoFrame, SsaoPlan};
use crate::core::quality::QualityPreset;
use crate::scene::authoring::default_visual_correction;
use crate::scene::Scene;

fn scene_with(edit: impl FnOnce(&mut crate::components::VisualCorrectionComponent)) -> Scene {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".to_string());
    let mut vc = default_visual_correction();
    edit(&mut vc);
    scene.world.set_visual_correction(id, Some(vc));
    scene
}

#[test]
fn a_default_volume_runs_ssao_on_medium_and_high_only() {
    let scene = scene_with(|_| {});
    assert_eq!(SsaoPlan::for_scene(&scene, QualityPreset::Low), None);
    let medium = SsaoPlan::for_scene(&scene, QualityPreset::Medium).expect("on");
    assert_eq!(medium.tier, QualityPreset::Medium.ssao().unwrap());
    assert!(SsaoPlan::for_scene(&scene, QualityPreset::High).is_some());
}

#[test]
fn nothing_runs_without_an_active_volume_or_with_ao_off() {
    let high = QualityPreset::High;
    assert_eq!(SsaoPlan::for_scene(&Scene::new(), high), None);
    assert_eq!(
        SsaoPlan::for_scene(&scene_with(|vc| vc.active = false), high),
        None
    );
    assert_eq!(
        SsaoPlan::for_scene(&scene_with(|vc| vc.ssao.active = false), high),
        None
    );
    // Zero intensity is invisible, so it costs nothing either.
    assert_eq!(
        SsaoPlan::for_scene(&scene_with(|vc| vc.ssao.intensity = 0.0), high),
        None
    );
}

#[test]
fn the_uniform_carries_the_look_and_the_tier() {
    let scene = scene_with(|vc| {
        vc.ssao.radius = 0.75;
        vc.ssao.intensity = 2.0;
    });
    let plan = SsaoPlan::for_scene(&scene, QualityPreset::Medium).unwrap();
    let view_proj = directx::perspective(1.0, 1.5, 0.1, 100.0);
    let frame = SsaoFrame {
        plan,
        view_proj,
        camera_pos: Vec3::new(1.0, 2.0, 3.0),
        camera_forward: Vec3::new(0.0, 0.0, -2.0),
    };
    let u = frame.uniform();
    assert_eq!(u.params, [0.75, 2.0, 8.0, 2.0]);
    assert_eq!(u.camera_pos, [1.0, 2.0, 3.0, 0.0]);
    assert_eq!(u.camera_fwd, [0.0, 0.0, -1.0, 0.0], "forward is normalized");
    let round_trip = Mat4::from_cols_array(&u.inv_view_proj) * view_proj;
    assert!(round_trip.abs_diff_eq(Mat4::IDENTITY, 1e-4));
}
