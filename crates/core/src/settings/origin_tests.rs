use serde_json::json;

use super::{OriginSettings, DEFAULT_THRESHOLD, ORIGIN_STEP};

#[test]
fn it_is_off_by_default_at_four_kilometres() {
    let s = OriginSettings::default();
    assert!(!s.enabled);
    assert_eq!(s.threshold, DEFAULT_THRESHOLD);
}

#[test]
fn a_file_turns_it_on_and_moves_the_threshold() {
    let (s, ignored) = OriginSettings::read_json(&json!({"enabled": true, "threshold": 8000}));
    assert!(ignored.is_empty(), "{ignored:?}");
    assert!(s.enabled);
    assert_eq!(s.threshold, 8000.0);
}

#[test]
fn a_threshold_under_one_step_or_not_a_number_is_refused() {
    for bad in [
        json!(ORIGIN_STEP - 1.0),
        json!(-5),
        json!("far"),
        json!(null),
    ] {
        let (s, ignored) = OriginSettings::read_json(&json!({"threshold": bad}));
        assert_eq!(s.threshold, DEFAULT_THRESHOLD, "{bad} keeps the default");
        assert_eq!(ignored.len(), 1, "{bad} is named: {ignored:?}");
    }
}

#[test]
fn an_unknown_key_is_named_and_the_rest_kept() {
    let (s, ignored) = OriginSettings::read_json(&json!({"enabled": true, "step": 10}));
    assert!(s.enabled);
    assert!(ignored[0].contains("unknown key 'step'"), "{ignored:?}");
}

#[test]
fn the_step_is_a_power_of_two_on_the_navmesh_tiles() {
    assert!((ORIGIN_STEP as u32).is_power_of_two());
    assert_eq!(
        ORIGIN_STEP % 32.0,
        0.0,
        "a whole number of 32 m navmesh tiles"
    );
}
