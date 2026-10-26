//! The shared post-processing ops' clamps and refusals.

use super::*;
use crate::components::{DepthOfField, PostProcessingComponent};
use crate::scene::Scene;

fn scene_with_vc() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".to_string());
    let c = PostProcessingComponent {
        bloom: crate::components::Bloom {
            active: false,
            threshold: 1.0,
            ..Default::default()
        },
        ssr: crate::components::Ssr {
            active: false,
            quality: "Medium".to_string(),
            temporal_upsampling: false,
        },
        ..crate::scene::authoring::default_post_processing()
    };
    scene.world.set_post_processing(id, Some(c));
    (scene, id)
}

#[test]
fn ops_write_through_with_clamps() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    set_bloom_intensity(&mut e, -3.0);
    set_bloom_threshold(&mut e, -1.0);
    set_gamma(&mut e, -5.0);
    set_tonemap(&mut e, Tonemap::Reinhard);
    set_ssr_quality(&mut e, "High".to_string());
    set_ssr_temporal_upsampling(&mut e, true);
    set_shadow_cascades(&mut e, 9);
    set_shadow_distance(&mut e, -4.0);
    set_ssao_active(&mut e, false);
    set_ssao_radius(&mut e, -1.0);
    set_ssao_intensity(&mut e, 9.0);
    set_white_balance(&mut e, 250.0, f32::NAN);
    set_wheel(&mut e, Wheel::Gain, [2.0, 0.5, -1.0], -3.0);
    set_hue_shift(&mut e, 400.0);
    let vc = &*e;
    assert_eq!(vc.bloom.intensity, 0.0);
    assert_eq!(vc.bloom.threshold, 0.0);
    assert_eq!(
        vc.tonemapping.display_gamma, 0.01,
        "gamma clamps to its floor"
    );
    assert_eq!(vc.tonemapping.mode, Tonemap::Reinhard);
    assert_eq!(vc.ssr.quality, "High");
    assert!(vc.ssr.temporal_upsampling);
    assert_eq!(vc.shadows.cascades, 4, "cascades clamp to the map's layers");
    assert_eq!(
        vc.shadows.distance, 1.0,
        "shadow distance clamps to its floor"
    );
    assert!(!vc.ssao.active);
    assert_eq!(vc.ssao.radius, 0.01, "radius clamps to its floor");
    assert_eq!(vc.ssao.intensity, 4.0, "intensity clamps to its ceiling");
    let wb = vc.white_balance;
    assert_eq!(
        (wb.temperature, wb.tint),
        (100.0, 0.0),
        "NaN tint is neutral"
    );
    let gain = vc.lift_gamma_gain.gain;
    assert_eq!((gain.color, gain.offset), ([1.0, 0.5, 0.0], -1.0));
    assert_eq!(vc.color_adjustments.hue_shift, 180.0);
}

#[test]
fn exposure_ops_hold_their_ranges() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    set_exposure_limits(&mut e, 3.0, -20.0);
    set_exposure_compensation(&mut e, f32::INFINITY);
    set_adaptation_speed(&mut e, -1.0, 1e9);
    set_histogram_percent(&mut e, 120.0, 50.0);
    let ae = e.exposure.automatic;
    assert_eq!((ae.min_ev, ae.max_ev), (3.0, 3.0), "max floors at min");
    assert_eq!(ae.compensation, 0.0, "an infinite compensation is refused");
    assert_eq!((ae.speed_up, ae.speed_down), (0.0, 100.0));
    assert_eq!((ae.low_percent, ae.high_percent), (100.0, 100.0));
}

#[test]
fn lut_path_has_one_spelling_and_blank_clears() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    set_lut(&mut e, Some(" ./luts\\a.cube ".into()));
    assert_eq!(
        e.color_lookup.lut.as_deref(),
        Some("luts/a.cube"),
        "one spelling"
    );
    set_lut(&mut e, Some("  ".into()));
    set_lut_contribution(&mut e, 4.0);
    assert_eq!(
        (&e.color_lookup.lut, e.color_lookup.contribution),
        (&None, 1.0),
        "blank clears"
    );
}

#[test]
fn custom_effects_keep_order_and_refuse_paths() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    set_custom_effects(&mut e, vec!["crt".into(), "grain".into()]).unwrap();
    for bad in ["", "../etc", "a/b", "a\\b", ".hidden"] {
        let err = set_custom_effects(&mut e, vec!["ok".into(), bad.into()]).unwrap_err();
        assert!(err.contains(&format!("{bad:?}")), "{err}");
    }
    assert_eq!(
        e.custom_effects.names,
        ["crt", "grain"],
        "a refused write changes nothing"
    );
}

#[test]
fn lens_ops_clamp_and_refuse_non_finite_vectors() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    set_vignette_intensity(&mut e, 4.0);
    set_vignette_smoothness(&mut e, -1.0);
    set_vignette_color(&mut e, [0.5, f32::NAN, 0.0]);
    set_vignette_center(&mut e, [2.0, -1.0]);
    set_chromatic_aberration_intensity(&mut e, f32::INFINITY);
    set_film_grain_intensity(&mut e, -2.0);
    set_film_grain_response(&mut e, 0.25);
    assert_eq!((e.vignette.intensity, e.vignette.smoothness), (1.0, 0.01));
    assert_eq!(
        e.vignette.color, [0.0; 3],
        "a NaN colour keeps the current one"
    );
    assert_eq!(e.vignette.center, [1.0, 0.0]);
    assert_eq!(e.chromatic_aberration.intensity, 0.0, "infinity is refused");
    assert_eq!((e.film_grain.intensity, e.film_grain.response), (0.0, 0.25));
}

#[test]
fn dof_ops_clamp_and_keep_the_gaussian_ends_independent() {
    let (mut scene, id) = scene_with_vc();
    let mut e = scene.world.post_processing_mut(id).unwrap();
    // A tween of the start out past the end and back leaves the end authored.
    set_dof_gaussian_start(&mut e, 40.0);
    set_dof_gaussian_start(&mut e, 10.0);
    assert_eq!(e.depth_of_field.gaussian_end, 30.0, "the end stays put");
    set_dof_gaussian_end(&mut e, 5.0);
    assert_eq!(
        e.depth_of_field.gaussian_end, 5.0,
        "nor floors at the start"
    );
    set_dof_gaussian_end(&mut e, -1.0);
    assert_eq!(e.depth_of_field.gaussian_end, 0.0, "but not below zero");
    set_dof_gaussian_max_radius(&mut e, 9.0);
    set_dof_aperture(&mut e, f32::NAN);
    set_dof_focal_length(&mut e, 1000.0);
    set_dof_focus_distance(&mut e, -3.0);
    let d = e.depth_of_field;
    assert_eq!((d.gaussian_max_radius, d.aperture), (1.5, 1.0));
    assert_eq!((d.focal_length, d.focus_distance), (300.0, 0.1));
    set_dof_focus_distance(&mut e, f32::INFINITY);
    let far = DepthOfField::MAX_FOCUS_DISTANCE;
    assert_eq!(e.depth_of_field.focus_distance, far, "+inf is infinity");
    let knob = dof_knob("depth_of_field.focus_distance").unwrap();
    (knob.set)(&mut e, 2.5);
    assert_eq!((knob.get)(&e.depth_of_field), 2.5);
    assert!(dof_knob("depth_of_field.mode").is_none(), "numbers only");
}
