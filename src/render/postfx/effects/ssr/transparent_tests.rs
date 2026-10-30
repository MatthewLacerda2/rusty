//! Transparent SSR's switches (#1570), off the GPU: the march the base camera
//! hands its translucents, and which material traces.

use super::transparent::camera_march;
use crate::components::{MaterialAsset, PostProcessingComponent, RenderMode, Ssr};
use crate::core::settings::quality::EffectQuality;
use crate::core::settings::GraphicsSettings;
use crate::render::draw::uniforms::surface_uniform;
use crate::render::postfx::params::PostSource;

fn volume(steps: u32, step_length: f32) -> PostProcessingComponent {
    PostProcessingComponent {
        ssr: Ssr {
            quality: EffectQuality::Custom,
            steps,
            step_length,
            ..Ssr::default()
        },
        ..PostProcessingComponent::default()
    }
}

fn march(vc: &PostProcessingComponent, on: bool) -> [f32; 2] {
    let post = PostSource {
        camera: None,
        volume: Some(vc),
    };
    camera_march(post, GraphicsSettings::default(), on)
}

#[test]
fn the_base_camera_marches_ssrs_quality_only_while_the_view_runs_ssr() {
    let vc = volume(40, 0.3);
    assert_eq!(march(&vc, true), [40.0, 0.3]);
    assert_eq!(march(&vc, false), [0.0; 2], "SSR off: nothing traces");
    let none = PostSource::default();
    assert_eq!(
        camera_march(none, GraphicsSettings::default(), true),
        [0.0; 2]
    );
    // A preset written by hand is held to the march's range.
    assert_eq!(march(&volume(0, 99.0), true), [1.0, Ssr::MAX_STEP_LENGTH]);
}

#[test]
fn only_a_lit_transparent_with_receive_ssr_traces() {
    let water = MaterialAsset {
        render_mode: RenderMode::Transparent,
        receive_ssr: true,
        ..MaterialAsset::default()
    };
    assert_eq!(surface_uniform(true, Some(&water)).receive_ssr, 1);
    assert_eq!(surface_uniform(false, Some(&water)).receive_ssr, 0, "unlit");
    let opaque = MaterialAsset {
        render_mode: RenderMode::Opaque,
        ..water.clone()
    };
    assert_eq!(
        surface_uniform(true, Some(&opaque)).receive_ssr,
        0,
        "opaque"
    );
    let off = MaterialAsset {
        receive_ssr: false,
        ..water
    };
    assert_eq!(surface_uniform(true, Some(&off)).receive_ssr, 0);
}
