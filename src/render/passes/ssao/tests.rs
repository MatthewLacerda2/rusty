//! The SSAO plan (#436): when the passes run, and what the shader is told.

use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

use super::{SsaoFrame, SsaoPlan};
use crate::core::settings::quality::{EffectQuality, QualityPresets, QualityValues};
use crate::core::settings::GraphicsSettings;
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

/// The plan for the scene's main camera's volume, as the screen stack asks for it,
/// under the seeded preset called `preset`.
fn plan(scene: &Scene, preset: &str) -> Option<SsaoPlan> {
    let main = crate::scene::camera::main_camera(scene);
    let volume = main.and_then(|id| crate::scene::camera::camera_volume(scene, id));
    let graphics = GraphicsSettings {
        quality: values(preset),
        ..Default::default()
    };
    SsaoPlan::for_volume(volume.as_deref(), graphics)
}

fn values(preset: &str) -> QualityValues {
    QualityPresets::default().get(preset).unwrap().values
}

/// No preset gates SSAO (#1522): a volume with it on runs it on every one, at its
/// level's row of that preset.
#[test]
fn a_default_volume_runs_ssao_on_every_preset() {
    let scene = scene_with(|_| {});
    for preset in ["Low", "Medium", "High"] {
        let plan = plan(&scene, preset).expect("on");
        assert_eq!(plan.quality, values(preset).ssao.medium, "{preset}");
    }
}

/// Switching preset changes a `Low`-level volume's samples and leaves a `Custom`
/// one's alone; `Custom` reaches the pass as authored.
#[test]
fn a_preset_switch_moves_a_level_and_leaves_custom_alone() {
    let low = scene_with(|vc| vc.ssao.quality = EffectQuality::Low);
    let (a, b) = (plan(&low, "Low").unwrap(), plan(&low, "High").unwrap());
    assert_ne!(a.quality, b.quality, "the level follows the preset");
    let custom = scene_with(|vc| {
        vc.ssao.quality = EffectQuality::Custom;
        (vc.ssao.samples, vc.ssao.resolution_divisor) = (32, 1);
    });
    for preset in ["Low", "High"] {
        let q = plan(&custom, preset).unwrap().quality;
        assert_eq!((q.samples, q.resolution_divisor), (32, 1), "{preset}");
    }
}

#[test]
fn nothing_runs_without_an_active_volume_or_with_ao_off() {
    let high = "High";
    assert_eq!(plan(&Scene::new(), high), None);
    assert_eq!(plan(&scene_with(|vc| vc.active = false), high), None);
    assert_eq!(plan(&scene_with(|vc| vc.ssao.active = false), high), None);
    // Zero intensity is invisible, so it costs nothing either.
    assert_eq!(plan(&scene_with(|vc| vc.ssao.intensity = 0.0), high), None);
}

#[test]
fn the_uniform_carries_the_look_and_the_quality() {
    let scene = scene_with(|vc| {
        vc.ssao.radius = 0.75;
        vc.ssao.intensity = 2.0;
    });
    let plan = plan(&scene, "Medium").unwrap();
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

/// #1438: the player's `ssao` option off skips the pass.
#[test]
fn the_player_option_off_skips_ssao() {
    use crate::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
    let scene = scene_with(|_| {});
    let mut options = GraphicsOptions::default();
    options.set(GraphicsOption::Ssao, false);
    let graphics = GraphicsSettings {
        options,
        ..Default::default()
    };
    let main = crate::scene::camera::main_camera(&scene);
    let volume = main.and_then(|id| crate::scene::camera::camera_volume(&scene, id));
    assert_eq!(SsaoPlan::for_volume(volume.as_deref(), graphics), None);
}

/// #1578: the blur is timed apart from the occlusion, and the upsample with the blur
/// it finishes (the prepass has its own entry too: `timing::tests`).
#[test]
fn each_stage_is_timed_under_its_own_entry() {
    use super::Stage;
    use crate::render::timing::GpuPass;
    let timed = Stage::ALL.map(Stage::timed);
    assert_eq!(timed, [GpuPass::Ssao, GpuPass::SsaoBlur, GpuPass::SsaoBlur]);
}
