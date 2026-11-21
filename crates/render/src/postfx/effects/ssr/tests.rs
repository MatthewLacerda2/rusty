use super::{fade, march, ssr_enabled, SMOOTHEST};
use crate::postfx::params::PostSource;
use rusty_components::Ssr;
use rusty_core::settings::quality::{EffectQuality, QualityPresets};
use rusty_core::settings::GraphicsSettings;

#[test]
fn each_level_marches_more_steps_and_reaches_further() {
    for preset in &QualityPresets::default().presets {
        let graphics = GraphicsSettings {
            quality: preset.values,
            ..Default::default()
        };
        let at = |quality| {
            march(
                &Ssr {
                    quality,
                    ..Ssr::default()
                },
                graphics,
            )
        };
        let levels = [
            EffectQuality::Low,
            EffectQuality::Medium,
            EffectQuality::High,
        ]
        .map(at);
        for pair in levels.windows(2) {
            let (lo, hi) = (pair[0], pair[1]);
            assert!(lo.steps <= hi.steps, "{}: {lo:?} vs {hi:?}", preset.name);
            let reach =
                |m: rusty_core::settings::quality::SsrQuality| m.steps as f32 * m.step_length;
            assert!(reach(lo) <= reach(hi), "{}: {lo:?} vs {hi:?}", preset.name);
        }
    }
}

#[test]
fn custom_marches_as_authored() {
    let ssr = Ssr {
        quality: EffectQuality::Custom,
        steps: 40,
        step_length: 0.3,
        ..Ssr::default()
    };
    let m = march(&ssr, GraphicsSettings::default());
    assert_eq!((m.steps, m.step_length), (40, 0.3));
}

#[test]
fn ssr_fades_in_over_at_most_a_fifth_and_is_whole_by_point_nine() {
    assert_eq!(fade(0.3), [0.3, Ssr::SMOOTHNESS_FADE]);
    let [min, width] = fade(0.8);
    assert!((min + width - 0.9).abs() < 1e-6, "{width}");
    assert_eq!(fade(1.0)[1], 0.01, "a hard cut, never a division by zero");
    assert_eq!(fade(-2.0), [0.0, Ssr::SMOOTHNESS_FADE]);
}

#[test]
fn a_minimum_smoothness_nothing_reaches_runs_no_ssr() {
    let mut vc = rusty_scene::create::default_post_processing();
    let runs = |vc: &rusty_components::PostProcessingComponent| {
        let post = PostSource {
            volume: Some(vc),
            ..Default::default()
        };
        ssr_enabled(post, GraphicsSettings::default(), true)
    };
    vc.ssr.min_smoothness = 0.95;
    assert!(runs(&vc));
    vc.ssr.min_smoothness = SMOOTHEST;
    assert!(!runs(&vc));
}
