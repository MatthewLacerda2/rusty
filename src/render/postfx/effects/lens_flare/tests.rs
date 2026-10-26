//! Lens flare's gate and params, adapter-free.

use super::{flare_params, flare_size};
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
    assert!(flare_params(&cx(&vc, high), [0.1; 2], 1.0).is_some());
    let low = QualityPreset::Low.into();
    assert!(
        flare_params(&cx(&vc, low), [0.1; 2], 1.0).is_none(),
        "off on Low"
    );
    let mut no_bloom = flaring();
    no_bloom.bloom.active = false;
    assert!(flare_params(&cx(&no_bloom, high), [0.1; 2], 1.0).is_none());
    let mut options = high;
    options.options.set(GraphicsOption::Bloom, false);
    assert!(flare_params(&cx(&vc, options), [0.1; 2], 1.0).is_none());
    let mut options = high;
    options.options.set(GraphicsOption::LensFlare, false);
    assert!(flare_params(&cx(&vc, options), [0.1; 2], 1.0).is_none());
    let mut dark = flaring();
    dark.lens_flare.intensity = 0.0;
    assert!(flare_params(&cx(&dark, high), [0.1; 2], 1.0).is_none());
}

#[test]
fn a_loaded_file_is_clamped_where_the_ops_would() {
    let mut vc = flaring();
    vc.lens_flare.ghost_count = 99;
    vc.lens_flare.tint = [2.0, 0.5, -1.0];
    vc.lens_flare.intensity = 2.0;
    vc.lens_flare.halo_intensity = -3.0;
    let p = flare_params(&cx(&vc, QualityPreset::Medium.into()), [0.1; 2], 1.0).unwrap();
    assert_eq!(p.tint, [2.0, 1.0, 0.0, 8.0]);
    assert_eq!(p.strengths[1], 0.0);
}

#[test]
fn the_flare_is_half_the_bloom_buffer() {
    assert_eq!(flare_size((960, 540)), (480, 270));
    assert_eq!(flare_size((1, 1)), (1, 1));
}
