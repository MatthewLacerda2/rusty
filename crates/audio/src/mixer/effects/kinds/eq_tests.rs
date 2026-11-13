//! The EQ against sines (#1748): a peak band's gain at and away from its centre,
//! a shelf, and an EQ with every band off.

use super::super::super::param::{Fields, Value};
use super::super::super::{EffectParams, KindId};
use super::super::signal::{peak_db, run, sine, RATE};

/// An EQ whose first band is `shape` at `hz`, `db` and `q`.
fn one_band(shape: &str, hz: f64, db: f64, q: f64) -> [f32; 24] {
    let kind = KindId::named("eq").expect("eq");
    let n = |v| Value::Number(v);
    let band: Fields = vec![
        ("shape".into(), Value::Text(shape.into())),
        ("frequency".into(), n(hz)),
        ("gain".into(), n(db)),
        ("q".into(), n(q)),
    ];
    let fields = vec![("bands".to_string(), Value::List(vec![band]))];
    let none = |g: &str| Err(g.to_string());
    let base = EffectParams::defaults(kind);
    super::super::super::param::parse(kind, &fields, &base, &none)
        .expect("valid")
        .0
        .values
}

/// The gain the EQ at `v` puts on a sine at `hz`, in dB, once settled.
fn gain_at(v: &[f32; 24], hz: f32) -> f32 {
    let clean = sine(hz, 0.1, RATE as usize / 2);
    let mut s = clean.clone();
    run("eq", v, &mut s);
    let tail = s.len() - 9600;
    peak_db(&s[tail..]) - peak_db(&clean[tail..])
}

#[test]
fn a_peak_band_lifts_its_centre_and_leaves_the_far_bands_alone() {
    let v = one_band("peak", 1000.0, 12.0, 1.0);
    let centre = gain_at(&v, 1000.0);
    assert!((centre - 12.0).abs() < 0.3, "{centre} dB at 1 kHz");
    let far = gain_at(&v, 60.0);
    assert!(far.abs() < 0.3, "{far} dB at 60 Hz");
}

#[test]
fn a_low_shelf_lifts_the_lows_only() {
    let v = one_band("low_shelf", 200.0, 6.0, 0.707);
    assert!((gain_at(&v, 40.0) - 6.0).abs() < 0.3);
    assert!(gain_at(&v, 5000.0).abs() < 0.3);
}

#[test]
fn every_band_off_is_bit_transparent() {
    let kind = KindId::named("eq").expect("eq");
    let clean = sine(440.0, 0.5, 4800);
    let mut s = clean.clone();
    run("eq", &EffectParams::defaults(kind).values, &mut s);
    assert_eq!(s, clean);
}
