//! The SSR quality's march (#1521, #1522), off the GPU.

use super::march;
use crate::components::Ssr;
use crate::core::settings::quality::{EffectQuality, QualityPresets};
use crate::core::settings::GraphicsSettings;

/// On every seeded preset, Low marches fewer steps than Medium and Medium than
/// High, and each reaches at least as far as the one below it.
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
                |m: crate::core::settings::quality::SsrQuality| m.steps as f32 * m.step_length;
            assert!(reach(lo) <= reach(hi), "{}: {lo:?} vs {hi:?}", preset.name);
        }
    }
}

/// `Custom` marches the volume's own steps and length, whatever the preset.
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
