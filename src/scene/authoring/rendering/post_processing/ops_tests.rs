//! The post effects' ops: their clamps and refusals.

use super::*;
use crate::components::PostProcessingComponent;
use crate::components::{Tonemap, Wheel};
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
    bloom::set_intensity(&mut e, -3.0);
    bloom::set_threshold(&mut e, -1.0);
    grading::tonemapping::set_display_gamma(&mut e, -5.0);
    grading::tonemapping::set_mode(&mut e, Tonemap::Reinhard);
    ssr::set_quality(&mut e, "High".to_string());
    ssr::set_temporal_upsampling(&mut e, true);
    volume::set_shadow_cascades(&mut e, 9);
    volume::set_shadow_distance(&mut e, -4.0);
    volume::set_ssao_active(&mut e, false);
    volume::set_ssao_radius(&mut e, -1.0);
    volume::set_ssao_intensity(&mut e, 9.0);
    grading::white_balance::set(&mut e, 250.0, f32::NAN);
    grading::lift_gamma_gain::set_wheel(&mut e, Wheel::Gain, [2.0, 0.5, -1.0], -3.0);
    grading::color_adjustments::set_hue_shift(&mut e, 400.0);
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
    exposure::set_limits(&mut e, 3.0, -20.0);
    exposure::set_compensation(&mut e, f32::INFINITY);
    exposure::set_adaptation_speed(&mut e, -1.0, 1e9);
    exposure::set_histogram_percent(&mut e, 120.0, 50.0);
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
    grading::color_lookup::set_lut(&mut e, Some(" ./luts\\a.cube ".into()));
    assert_eq!(
        e.color_lookup.lut.as_deref(),
        Some("luts/a.cube"),
        "one spelling"
    );
    grading::color_lookup::set_lut(&mut e, Some("  ".into()));
    grading::color_lookup::set_contribution(&mut e, 4.0);
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
    custom_effects::set_names(&mut e, vec!["crt".into(), "grain".into()]).unwrap();
    for bad in ["", "../etc", "a/b", "a\\b", ".hidden"] {
        let err = custom_effects::set_names(&mut e, vec!["ok".into(), bad.into()]).unwrap_err();
        assert!(err.contains(&format!("{bad:?}")), "{err}");
    }
    assert_eq!(
        e.custom_effects.names,
        ["crt", "grain"],
        "a refused write changes nothing"
    );
}
