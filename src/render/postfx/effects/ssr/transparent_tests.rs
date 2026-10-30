//! Transparent SSR's switches (#1570), off the GPU: the march the base camera
//! hands its translucents, and which material traces.

use super::transparent::{camera_march, traces, MIN_SMOOTHNESS};
use crate::asset::MapRef;
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
        roughness: 0.04,
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

#[test]
fn a_material_too_rough_to_ever_reflect_traces_nothing() {
    let glass = |roughness| MaterialAsset {
        render_mode: RenderMode::Transparent,
        receive_ssr: true,
        roughness,
        ..MaterialAsset::default()
    };
    assert!(traces(&glass(0.04)), "still water");
    assert!(
        !traces(&glass(0.2)),
        "frosted glass: a fade of 0 everywhere"
    );
    // A roughness map multiplies the factor down: some pixel may still reflect.
    let mapped = MaterialAsset {
        roughness_map: Some(MapRef::from("ripples.png")),
        ..glass(0.6)
    };
    assert!(traces(&mapped));
    assert_eq!(surface_uniform(true, Some(&glass(0.6))).receive_ssr, 0);
}

#[test]
fn the_minimum_smoothness_is_one_number_in_rust_and_wgsl() {
    let wgsl = include_str!("../../../../../engine/shaders/lighting/ssr_surface.wgsl");
    let line = format!("const MIN_SMOOTHNESS: f32 = {MIN_SMOOTHNESS:.1};");
    assert!(
        wgsl.contains(&line),
        "ssr_surface.wgsl should declare `{line}`"
    );
}
