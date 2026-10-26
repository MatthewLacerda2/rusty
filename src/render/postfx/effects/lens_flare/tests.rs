//! Lens flare's gate and params, adapter-free.

use super::{flare_params, flare_size};
use crate::components::rendering::post_processing::LensFlare;
use crate::components::PostProcessingComponent;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::effects::EffectCx;
use crate::render::postfx::params::PostSource;
use crate::scene::authoring::default_post_processing;

fn cx(vc: &PostProcessingComponent, graphics: GraphicsSettings) -> EffectCx<'_> {
    EffectCx {
        post: PostSource {
            camera: None,
            volume: Some(vc),
        },
        graphics,
        full: true,
    }
}

fn flaring() -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.lens_flare.active = true;
    vc
}

#[test]
fn it_flares_only_while_bloom_runs_off_the_low_tier() {
    let vc = flaring();
    let high = GraphicsSettings::from(QualityPreset::High);
    assert!(flare_params(&cx(&vc, high), 1.0).is_some());
    let low = QualityPreset::Low.into();
    assert!(flare_params(&cx(&vc, low), 1.0).is_none(), "off on Low");
    let mut no_bloom = flaring();
    no_bloom.bloom.active = false;
    assert!(flare_params(&cx(&no_bloom, high), 1.0).is_none());
    let mut options = high;
    options.options.set(GraphicsOption::Bloom, false);
    assert!(flare_params(&cx(&vc, options), 1.0).is_none());
    let mut options = high;
    options.options.set(GraphicsOption::LensFlare, false);
    assert!(flare_params(&cx(&vc, options), 1.0).is_none());
}

#[test]
fn a_flare_that_can_add_nothing_does_not_run() {
    let high = GraphicsSettings::from(QualityPreset::High);
    let dark = |edit: fn(&mut LensFlare)| {
        let mut vc = flaring();
        edit(&mut vc.lens_flare);
        flare_params(&cx(&vc, high), 1.0).is_none()
    };
    assert!(dark(|f| f.intensity = 0.0));
    assert!(dark(|f| f.tint = [0.0; 3]));
    assert!(dark(|f| (f.ghost_count, f.halo_intensity) = (0, 0.0)));
    assert!(dark(|f| (f.ghost_intensity, f.halo_intensity) = (0.0, 0.0)));
    let streaks_only = |f: &mut LensFlare| {
        (f.ghost_count, f.halo_intensity, f.streak_intensity) = (0, 0.0, 1.0);
    };
    assert!(!dark(streaks_only), "the streaks alone draw");
    assert!(!dark(|f| f.ghost_count = 0), "the halo alone draws");
}

#[test]
fn a_loaded_file_is_clamped_where_the_ops_would() {
    let mut vc = flaring();
    vc.lens_flare.ghost_count = 99;
    vc.lens_flare.tint = [2.0, 0.5, -1.0];
    vc.lens_flare.intensity = 2.0;
    vc.lens_flare.halo_intensity = -3.0;
    let p = flare_params(&cx(&vc, QualityPreset::Medium.into()), 1.0).unwrap();
    assert_eq!(p.tint, [2.0, 1.0, 0.0, 8.0]);
    assert_eq!(p.strengths[1], 0.0);
}

#[test]
fn the_flare_is_half_the_bloom_buffer() {
    assert_eq!(flare_size((960, 540)), (480, 270));
    assert_eq!(flare_size((1, 1)), (1, 1));
}

#[test]
fn the_shader_draws_as_many_ghosts_as_the_component_allows() {
    let wgsl = include_str!("../../../../../engine/shaders/postfx/lens_flare.wgsl");
    let max = LensFlare::MAX_GHOSTS;
    let bound = format!("const MAX_GHOSTS: i32 = {max};");
    assert!(wgsl.contains(&bound), "the WGSL's bound is `{bound}`");
    // `ghost_scale` and `ghost_tint`: one case per ghost but the last, the default's.
    let cases = wgsl.matches("case ").count() as u32;
    assert_eq!(cases, 2 * (max - 1), "each switch covers every ghost");
}
