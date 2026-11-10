//! Transparent SSR's switches (#1570), off the GPU: the march the base camera
//! hands its translucents, and which material traces.

use super::transparent::{camera_march, traces, SHARP_SMOOTHNESS};
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

fn march(vc: &PostProcessingComponent, on: bool) -> [f32; 4] {
    let post = PostSource {
        camera: None,
        volume: Some(vc),
    };
    camera_march(post, GraphicsSettings::default(), on)
}

#[test]
fn the_base_camera_marches_ssrs_quality_only_while_the_view_runs_ssr() {
    let vc = volume(40, 0.3);
    let on = march(&vc, true);
    assert_eq!(
        on[..3],
        [40.0, 0.3, SHARP_SMOOTHNESS],
        "whole from 0.9: {on:?}"
    );
    assert!((on[2] + on[3] - 0.9).abs() < 1e-6, "whole from 0.9: {on:?}");
    assert_eq!(march(&vc, false), [0.0; 4], "SSR off: nothing traces");
    let none = PostSource::default();
    assert_eq!(
        camera_march(none, GraphicsSettings::default(), true),
        [0.0; 4]
    );
    // A preset written by hand is held to the march's range.
    let far = march(&volume(0, 99.0), true);
    assert_eq!([far[0], far[1]], [1.0, Ssr::MAX_STEP_LENGTH]);
}

#[test]
fn a_translucent_reflects_sharp_so_never_below_the_sharp_floor() {
    // The volume's default reflects opaques to roughness 0.7; a translucent, whose
    // hit is sharp, keeps the floor. A stricter volume is followed.
    let mut vc = volume(40, 0.3);
    assert!(vc.ssr.min_smoothness < SHARP_SMOOTHNESS);
    assert_eq!(march(&vc, true)[2], SHARP_SMOOTHNESS);
    vc.ssr.min_smoothness = 0.9;
    assert_eq!(march(&vc, true)[2], 0.9);
}

#[test]
fn receive_ssr_defaults_by_render_mode_and_an_explicit_value_wins() {
    let water = MaterialAsset {
        render_mode: RenderMode::Transparent,
        receive_ssr: Some(true),
        roughness: 0.04,
        ..MaterialAsset::default()
    };
    let lane = |m: &MaterialAsset| surface_uniform(true, Some(m)).receive_ssr;
    assert_eq!(lane(&water), 1);
    assert_eq!(surface_uniform(false, Some(&water)).receive_ssr, 0, "unlit");
    let unset = MaterialAsset {
        receive_ssr: None,
        ..water.clone()
    };
    assert_eq!(lane(&unset), 0, "a translucent traces only when asked");
    let opaque = MaterialAsset {
        render_mode: RenderMode::Opaque,
        ..unset
    };
    assert_eq!(lane(&opaque), 1, "an opaque surface takes SSR by default");
    let opted_out = MaterialAsset {
        receive_ssr: Some(false),
        ..opaque
    };
    assert_eq!(lane(&opted_out), 0, "#2004: an opaque surface may opt out");
    assert_eq!(surface_uniform(true, None).receive_ssr, 1, "no material");
}

#[test]
fn a_material_too_rough_to_ever_reflect_traces_nothing() {
    let glass = |roughness| MaterialAsset {
        render_mode: RenderMode::Transparent,
        receive_ssr: Some(true),
        roughness,
        ..MaterialAsset::default()
    };
    assert!(traces(&glass(0.04)), "still water");
    assert!(traces(&glass(0.25)), "a faint sharp reflection");
    assert!(
        !traces(&glass(0.3)),
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
