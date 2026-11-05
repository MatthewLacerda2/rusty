//! The plan's ranges, the grid, the camera's slot and the history's weight (#1703),
//! without a GPU.

use super::params::{Built, VolumetricParams};
use super::{grid, VolumetricPlan, GHOST_FRAMES, HISTORY_WEIGHT, VOLUME_NEAR};
use crate::components::{PostProcessingComponent, VolumetricFog};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::{EffectQuality, VolumetricFogQuality};
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::PostSource;

fn volume(fog: VolumetricFog) -> PostProcessingComponent {
    PostProcessingComponent {
        volumetric_fog: fog,
        ..Default::default()
    }
}

fn plan(fog: VolumetricFog) -> Option<VolumetricPlan> {
    plan_under(fog, GraphicsSettings::default(), true)
}

fn plan_under(fog: VolumetricFog, g: GraphicsSettings, full: bool) -> Option<VolumetricPlan> {
    let vc = volume(fog);
    let post = PostSource {
        camera: None,
        volume: Some(&vc),
    };
    VolumetricPlan::for_view(post, g, full)
}

fn on() -> VolumetricFog {
    VolumetricFog {
        active: true,
        ..VolumetricFog::default()
    }
}

#[test]
fn off_by_default_and_off_where_the_option_or_the_view_says_so() {
    assert_eq!(plan(VolumetricFog::default()), None);
    assert!(plan(on()).is_some());
    assert_eq!(plan_under(on(), GraphicsSettings::default(), false), None);
    let mut g = GraphicsSettings::default();
    g.options.set(GraphicsOption::VolumetricFog, false);
    assert_eq!(plan_under(on(), g, true), None);
    let none = PostSource {
        camera: None,
        volume: None,
    };
    assert_eq!(
        VolumetricPlan::for_view(none, GraphicsSettings::default(), true),
        None
    );
}

#[test]
fn every_value_is_brought_into_range_and_a_nan_takes_the_default() {
    let wild = |v: f32| VolumetricFog {
        mean_free_path: v,
        anisotropy: v,
        base_height: v,
        maximum_height: v,
        depth_extent: v,
        global_light_probe_dimmer: v,
        albedo: [v; 3],
        ..on()
    };
    let defaults = plan(on()).unwrap();
    let nan = plan(wild(f32::NAN)).unwrap();
    assert_eq!(nan.extinction, defaults.extinction);
    assert_eq!(nan.depth_extent, defaults.depth_extent);
    assert_eq!(
        (nan.anisotropy, nan.ambient, nan.albedo),
        (0.0, 1.0, [1.0; 3])
    );
    for v in [0.0, -1e30, 1e30, f32::INFINITY] {
        let p = plan(wild(v)).unwrap();
        let finite = [
            p.extinction,
            p.anisotropy,
            p.base_height,
            p.falloff,
            p.depth_extent,
        ];
        assert!(finite.iter().all(|x| x.is_finite()), "{v}: {p:?}");
        assert!(
            (1e-4..=1.0).contains(&p.extinction),
            "{v}: {}",
            p.extinction
        );
        assert!(p.anisotropy.abs() <= VolumetricFog::MAX_ANISOTROPY);
        assert!((4.0..=1000.0).contains(&p.depth_extent));
        assert!(p.falloff > 0.0 && p.falloff <= 100.0, "{v}: {}", p.falloff);
    }
}

#[test]
fn the_grid_is_nine_sixteenths_up_and_clamped() {
    let q = |resolution, slices| VolumetricFogQuality { resolution, slices };
    assert_eq!(grid(q(160, 64)), [160, 90, 64]);
    assert_eq!(grid(q(0, 0)), [16, 9, 16]);
    assert_eq!(grid(q(u32::MAX, u32::MAX)), [256, 144, 128]);
    let custom = VolumetricFog {
        quality: EffectQuality::Custom,
        resolution: 80,
        slices: 32,
        ..on()
    };
    assert_eq!(plan(custom).unwrap().grid, [80, 45, 32]);
    assert_eq!(plan(custom).unwrap().froxels(), 80 * 45 * 32);
}

#[test]
fn the_camera_reads_no_volume_without_a_plan() {
    assert_eq!(VolumetricPlan::camera_slot(None), [0.0; 4]);
    let p = plan(on()).unwrap();
    let slot = VolumetricPlan::camera_slot(Some(&p));
    assert_eq!(slot, [1.0, p.grid[2] as f32, 64.0, VOLUME_NEAR]);
}

#[test]
fn a_moved_light_leaves_under_five_percent_after_the_ghost_frames() {
    assert!(HISTORY_WEIGHT.powi(GHOST_FRAMES as i32) < 0.05);
    assert!(HISTORY_WEIGHT.powi(GHOST_FRAMES as i32 - 1) >= 0.05);
    let p = plan(on()).unwrap();
    assert_eq!(
        VolumetricParams::new(&p, (3, [[0.0; 4]; 2]), None).weight(),
        1.0
    );
    let built = Built {
        view_proj: glam::Mat4::IDENTITY,
        view_z: glam::Vec4::ZERO,
        volumetric: [1.0; 4],
    };
    let w = VolumetricParams::new(&p, (3, [[0.0; 4]; 2]), Some(built)).weight();
    assert!((w - (1.0 - HISTORY_WEIGHT)).abs() < 1e-6);
}
