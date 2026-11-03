//! The scene-wide diffuse model (#1698): its response and the probes' SH reshaping
//! of it agree, so a probe-lit object answers a Baked lamp as a realtime one would.

use glam::Vec3;

use super::DiffuseModel;
use crate::scene::lighting::probe_fill::fibonacci_sphere;
use crate::scene::lighting::sh::Sh9;
use crate::scene::lighting::LightingSettings;

#[test]
fn lambert_is_the_default_and_a_settings_block_without_it_reads_as_lambert() {
    assert_eq!(
        LightingSettings::default().diffuse_model,
        DiffuseModel::Lambert
    );
    let old: LightingSettings = serde_json::from_str(r#"{"lightmaps":{"samples":7}}"#).unwrap();
    assert_eq!(old.diffuse_model, DiffuseModel::Lambert);
}

#[test]
fn the_responses_are_lambert_and_the_squared_wrap() {
    let (l, h) = (DiffuseModel::Lambert, DiffuseModel::HalfLambert);
    assert_eq!(
        (l.response(1.0), l.response(0.0), l.response(-0.5)),
        (1.0, 0.0, 0.0)
    );
    assert_eq!(
        (h.response(1.0), h.response(0.0), h.response(-1.0)),
        (1.0, 0.25, 0.0)
    );
    assert!((h.response(0.5) - 0.5625).abs() < 1e-6);
    // Out of range (a denormalised normal) stays in [0, 1].
    assert_eq!((h.response(3.0), h.response(-3.0)), (1.0, 0.0));
}

#[test]
fn names_round_trip_and_an_unknown_one_is_none() {
    for m in DiffuseModel::ALL {
        assert_eq!(DiffuseModel::from_name(m.name()), Some(m));
    }
    assert_eq!(DiffuseModel::from_name("Wrap"), None);
}

#[test]
fn a_band_scaled_delta_reads_back_as_the_models_response() {
    let light = Vec3::new(0.3, 0.8, -0.5).normalize();
    let radiance = Vec3::new(2.0, 1.0, 0.5);
    for model in DiffuseModel::ALL {
        let mut sh = Sh9::zero();
        sh.add_sample(light, radiance, 1.0);
        let sh = sh.band_scaled(model.sh_band_scale());
        for n in fibonacci_sphere(64) {
            let want = radiance * model.response(n.dot(light));
            let got = sh.eval(n);
            // Lambert is only L2-accurate; the wrap, a quadratic, is exact.
            let tol = if model == DiffuseModel::Lambert {
                0.25
            } else {
                1e-3
            };
            assert!(
                (got - want).abs().max_element() < tol * 2.0,
                "{model:?} {n}: {got} vs {want}"
            );
        }
    }
}
