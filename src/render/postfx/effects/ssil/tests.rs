//! SSIL's plan and uniform (#1750), off the GPU: when it runs, what it is clamped
//! to, and what a cut tells the shader.

use glam::{Mat4, Vec3};

use super::{Previous, SsilFrame, SsilPlan};
use crate::components::{PostProcessingComponent, Ssil};
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::{EffectQuality, QualityPresets};
use crate::core::settings::GraphicsSettings;
use crate::render::motion::HistoryKey;
use crate::render::postfx::params::PostSource;
use crate::scene::Scene;

fn plan_of(ssil: Ssil, graphics: GraphicsSettings, full: bool) -> Option<SsilPlan> {
    let volume = PostProcessingComponent {
        ssil,
        ..Default::default()
    };
    let post = PostSource {
        camera: None,
        volume: Some(&volume),
    };
    SsilPlan::for_view(post, graphics, full)
}

fn on() -> Ssil {
    Ssil {
        active: true,
        ..Ssil::default()
    }
}

/// Off by default, and off without a volume, at intensity 0, without full post-FX
/// or with the player's option off; on otherwise.
#[test]
fn ssil_runs_only_when_turned_on_and_allowed() {
    let g = GraphicsSettings::default();
    assert!(
        plan_of(Ssil::default(), g, true).is_none(),
        "off by default"
    );
    assert!(plan_of(on(), g, true).is_some());
    assert!(plan_of(on(), g, false).is_none(), "a display-only view");
    let zero = Ssil {
        intensity: 0.0,
        ..on()
    };
    assert!(plan_of(zero, g, true).is_none());
    let mut forbidden = g;
    forbidden.options.set(GraphicsOption::Ssil, false);
    assert!(plan_of(on(), forbidden, true).is_none());
    let post = PostSource {
        camera: None,
        volume: None,
    };
    assert!(SsilPlan::for_view(post, g, true).is_none());
}

/// Zero, huge and NaN values reach the GPU inside their range: a NaN intensity
/// runs nothing, a NaN radius runs at the default, huge ones at the maximum.
#[test]
fn intensity_and_radius_are_bounded() {
    let g = GraphicsSettings::default();
    let with = |intensity, radius| {
        plan_of(
            Ssil {
                intensity,
                radius,
                ..on()
            },
            g,
            true,
        )
    };
    assert!(with(f32::NAN, 1.0).is_none());
    let huge = with(1e30, 1e30).unwrap();
    assert_eq!(
        (huge.intensity, huge.radius),
        (Ssil::MAX_INTENSITY, Ssil::MAX_RADIUS)
    );
    let tiny = with(1.0, 0.0).unwrap();
    assert_eq!(tiny.radius, Ssil::MIN_RADIUS);
    let nan = with(1.0, f32::NAN).unwrap();
    assert_eq!(nan.radius, Ssil::default().radius);
    let custom = Ssil {
        quality: EffectQuality::Custom,
        samples: 1000,
        resolution_divisor: 0,
        ..on()
    };
    let q = plan_of(custom, g, true).unwrap().quality;
    assert_eq!((q.samples, q.resolution_divisor), (Ssil::MAX_SAMPLES, 1));
}

/// A level takes the running preset's row; every seeded level gathers at half
/// resolution or finer, and more samples from Low to High.
#[test]
fn a_level_takes_the_presets_row() {
    for preset in &QualityPresets::default().presets {
        let g = GraphicsSettings {
            quality: preset.values,
            ..Default::default()
        };
        let at = |quality| {
            let ssil = Ssil { quality, ..on() };
            plan_of(ssil, g, true).unwrap().quality
        };
        let [lo, mid, hi] = [
            EffectQuality::Low,
            EffectQuality::Medium,
            EffectQuality::High,
        ]
        .map(at);
        assert_eq!(mid, preset.values.ssil.medium, "{}", preset.name);
        assert!(
            lo.samples < mid.samples && mid.samples < hi.samples,
            "{}",
            preset.name
        );
        assert!(hi.resolution_divisor <= 2, "{}", preset.name);
    }
}

fn frame() -> SsilFrame {
    SsilFrame {
        plan: plan_of(on(), GraphicsSettings::default(), true).unwrap(),
        view_proj: glam::camera::rh::proj::directx::perspective(1.0, 1.0, 0.1, 100.0),
        eye: Vec3::new(1.0, 2.0, 3.0),
        forward: Vec3::new(0.0, 0.0, -2.0),
        key: HistoryKey::new(&Scene::new(), None),
    }
}

/// A cut tells the shader it has no history (and reprojects through this frame's
/// camera); with one, last frame's camera; the forward axis is normalised.
#[test]
fn the_uniform_says_whether_there_is_a_history() {
    let f = frame();
    let cut = f.uniform(None);
    assert_eq!(cut.camera_fwd, [0.0, 0.0, -1.0, 0.0]);
    assert_eq!(cut.prev_view_proj, f.view_proj.to_cols_array());
    let prev = Previous {
        view_proj: Mat4::IDENTITY,
        eye: Vec3::X,
    };
    let held = f.uniform(Some(prev));
    assert_eq!(held.camera_fwd[3], 1.0);
    assert_eq!(held.prev_view_proj, Mat4::IDENTITY.to_cols_array());
    assert_eq!(held.prev_camera_pos, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(held.params, [2.0, 1.0, 8.0, 2.0]);
}
