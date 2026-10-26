//! The SSAO plan (#436): when the passes run, and what the shader is told.

use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

use super::{SsaoFrame, SsaoPlan};
use crate::core::settings::quality::QualityPreset;
use crate::scene::authoring::default_post_processing;
use crate::scene::Scene;

fn scene_with(edit: impl FnOnce(&mut crate::components::PostProcessingComponent)) -> Scene {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".to_string());
    let mut vc = default_post_processing();
    edit(&mut vc);
    scene.world.set_post_processing(id, Some(vc));
    // A volume rides on its camera (#1434): that camera is the view's.
    scene
        .world
        .set_camera(id, Some(crate::components::CameraComponent::default()));
    scene
}

/// The plan for the scene's main camera's volume, as the screen stack asks for it.
fn plan(scene: &Scene, quality: QualityPreset) -> Option<SsaoPlan> {
    let main = crate::scene::camera::main_camera(scene);
    let volume = main.and_then(|id| crate::scene::camera::camera_volume(scene, id));
    SsaoPlan::for_volume(volume.as_deref(), quality.into())
}

#[test]
fn a_default_volume_runs_ssao_on_medium_and_high_only() {
    let scene = scene_with(|_| {});
    assert_eq!(plan(&scene, QualityPreset::Low), None);
    let medium = plan(&scene, QualityPreset::Medium).expect("on");
    assert_eq!(medium.tier, QualityPreset::Medium.ssao().unwrap());
    assert!(plan(&scene, QualityPreset::High).is_some());
}

#[test]
fn nothing_runs_without_an_active_volume_or_with_ao_off() {
    let high = QualityPreset::High;
    assert_eq!(plan(&Scene::new(), high), None);
    assert_eq!(plan(&scene_with(|vc| vc.active = false), high), None);
    assert_eq!(plan(&scene_with(|vc| vc.ssao.active = false), high), None);
    // Zero intensity is invisible, so it costs nothing either.
    assert_eq!(plan(&scene_with(|vc| vc.ssao.intensity = 0.0), high), None);
}

#[test]
fn the_uniform_carries_the_look_and_the_tier() {
    let scene = scene_with(|vc| {
        vc.ssao.radius = 0.75;
        vc.ssao.intensity = 2.0;
    });
    let plan = plan(&scene, QualityPreset::Medium).unwrap();
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

/// #1438: the player's `ssao` option off skips the pass even on High.
#[test]
fn the_player_option_off_skips_ssao() {
    use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
    let scene = scene_with(|_| {});
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssao, false);
    let graphics = crate::core::settings::GraphicsSettings {
        quality: QualityPreset::High,
        options,
    };
    let main = crate::scene::camera::main_camera(&scene);
    let volume = main.and_then(|id| crate::scene::camera::camera_volume(&scene, id));
    assert_eq!(SsaoPlan::for_volume(volume.as_deref(), graphics), None);
}
