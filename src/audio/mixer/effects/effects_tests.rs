//! The mixer's effects (#1748): Master's default limiter, a script's tables checked
//! and read back, a snapshot gliding a parameter, and sidechain cycles refused.

use super::*;
use crate::audio::mixer::{GroupPatch, Mixer};

fn n(k: &str, v: f64) -> (String, Value) {
    (k.to_string(), Value::Number(v))
}
fn t(k: &str, v: &str) -> (String, Value) {
    (k.to_string(), Value::Text(v.to_string()))
}
fn effect(kind: &str, rest: &[(String, Value)]) -> Fields {
    let mut f = vec![t("kind", kind)];
    f.extend(rest.iter().cloned());
    f
}
fn sfx(m: &Mixer) -> GroupId {
    m.find("SFX").expect("SFX")
}

#[test]
fn master_starts_with_a_limiter_at_minus_point_three_and_the_rest_with_nothing() {
    let m = Mixer::default();
    let master: Vec<_> = m.effects(GroupId::MASTER).iter().collect();
    assert_eq!(master.len(), 1);
    assert_eq!(master[0].kind.row().name, "limiter");
    assert_eq!(master[0].values[0], -0.3);
    assert_eq!(m.effects(sfx(&m)).iter().count(), 0);
}

#[test]
fn a_bad_kind_field_or_value_is_an_error_that_says_what_is_allowed() {
    let m = Mixer::default();
    let err = |tables: &[Fields]| m.parse_effects(tables).unwrap_err();
    assert!(err(&[effect("reverb", &[])]).contains("compressor, delay"));
    assert!(err(&[effect("limiter", &[n("celing", -1.0)])]).contains("ceiling"));
    assert!(err(&[effect("limiter", &[n("ceiling", 3.0)])]).contains("[-24, 0]"));
    assert!(err(&[effect("limiter", &[n("ceiling", f64::NAN)])]).contains("out of range"));
    assert!(err(&[effect("distortion", &[t("mode", "fuzz")])]).contains("bitcrush"));
    assert!(err(&[effect("compressor", &[t("sidechain", "Gunz")])]).contains("Music"));
    assert!(err(&[effect("compressor", &[n("sidechain", 1.0)])]).contains("group name"));
    assert!(err(&[vec![n("ratio", 2.0)]]).contains("kind"));
    assert!(err(&vec![effect("limiter", &[]); 9]).contains("at most 8"));
}

#[test]
fn a_chain_reads_back_with_every_parameter_and_the_eq_bands_nested() {
    let mut m = Mixer::default();
    let tables = [
        effect(
            "compressor",
            &[n("threshold", -18.0), t("sidechain", "Music")],
        ),
        effect(
            "eq",
            &[(
                "bands".to_string(),
                Value::List(vec![vec![t("shape", "peak"), n("gain", 3.0)]]),
            )],
        ),
    ];
    let chain = m.parse_effects(&tables).expect("valid");
    m.set_effects(sfx(&m), chain);
    let back = m.effect_fields(sfx(&m));
    assert!(back[0].contains(&n("threshold", -18.0)));
    assert!(back[0].contains(&n("ratio", 4.0)), "a default");
    assert!(back[0].contains(&t("sidechain", "Music")));
    let Some((_, Value::List(bands))) = back[1].iter().find(|(k, _)| k == "bands") else {
        panic!("bands nested: {:?}", back[1]);
    };
    assert_eq!(bands.len(), 6);
    assert!(bands[0].contains(&t("shape", "peak")) && bands[0].contains(&n("gain", 3.0)));
    assert!(bands[1].contains(&t("shape", "off")));
}

#[test]
fn a_snapshot_glides_an_effect_parameter_and_checks_the_kind() {
    let mut m = Mixer::default();
    let id = sfx(&m);
    let chain = m
        .parse_effects(&[effect("compressor", &[])])
        .expect("valid");
    m.set_effects(id, chain);
    let patch = m
        .parse_effect_patch(id, &[effect("compressor", &[n("threshold", -40.0)])])
        .expect("valid");
    assert!(m
        .parse_effect_patch(id, &[effect("limiter", &[])])
        .unwrap_err()
        .contains("compressor"));
    let keyed = [effect("compressor", &[t("sidechain", "Music")])];
    assert!(m
        .parse_effect_patch(id, &keyed)
        .unwrap_err()
        .contains("sidechain"));
    let snapshot = GroupPatch {
        effect_params: Some(patch),
        ..GroupPatch::default()
    };
    m.define_snapshot("Loud", vec![(id, snapshot)]);
    m.transition_to("Loud", 2.0, 0.0).expect("defined");
    m.advance(1.0, |_| false);
    let at = |m: &Mixer| m.effects(id).0[0].expect("one").values;
    assert_eq!(at(&m)[0], -20.0, "halfway");
    assert_eq!(at(&m)[1], 4.0, "an unnamed value holds");
    m.advance(2.0, |_| false);
    assert_eq!(at(&m)[0], -40.0);
}

#[test]
fn a_sidechain_cycle_is_dropped() {
    let mut m = Mixer::default();
    let (id, music) = (sfx(&m), m.find("Music").expect("Music"));
    let keyed = |m: &Mixer, from: &str| {
        m.parse_effects(&[effect("compressor", &[t("sidechain", from)])])
            .expect("valid")
    };
    for (from, why) in [("SFX", "itself"), ("Master", "an ancestor")] {
        let chain = keyed(&m, from);
        m.set_effects(id, chain);
        assert_eq!(m.effects(id).0[0].expect("kept").key(), None, "{why}");
    }
    let chain = keyed(&m, "SFX");
    m.set_effects(music, chain);
    assert_eq!(m.effects(music).0[0].expect("kept").key(), Some(id));
    let chain = keyed(&m, "Music");
    m.set_effects(id, chain);
    assert_eq!(m.effects(id).0[0].expect("kept").key(), None, "a loop");
}
