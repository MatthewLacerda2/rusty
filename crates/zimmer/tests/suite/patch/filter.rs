//! The filter stage as a *document*: what a recipe may write in it, and what it is refused for.

use rusty_zimmer::patch::{Filter, FilterKind, Slope};

#[test]
fn the_kinds_are_spelled_as_one_word() {
    for (kind, spelling) in [
        (FilterKind::Lowpass, "lowpass"),
        (FilterKind::Highpass, "highpass"),
    ] {
        let json = serde_json::to_string(&kind).expect("serialise");
        assert_eq!(json, format!("\"{spelling}\""));
    }
}

#[test]
fn the_hz_era_field_names_are_refused_rather_than_ignored() {
    for stale in ["env_amount", "vel_cutoff"] {
        let json = format!(r#"{{ "kind": "lowpass", "cutoff": 800, "{stale}": 3200 }}"#);
        let refused = serde_json::from_str::<Filter>(&json).expect_err(stale);
        let said = refused.to_string();
        assert!(said.contains(stale), "the offending word is named: {said}");
        assert!(
            said.contains("env_octaves") && said.contains("vel_octaves"),
            "and the ones that work are listed: {said}"
        );
    }
}

#[test]
fn a_depth_is_read_the_same_way_wherever_the_cutoff_sits() {
    for cutoff in [200.0, 6800.0] {
        let json = format!(r#"{{ "kind": "lowpass", "cutoff": {cutoff}, "env_octaves": 2 }}"#);
        let filter: Filter = serde_json::from_str(&json).expect("parses");
        assert_eq!(filter.env_octaves, 2.0);
    }
}

#[test]
fn every_mode_the_topology_yields_is_writable() {
    for (kind, spelling) in [
        (FilterKind::Bandpass, "bandpass"),
        (FilterKind::Notch, "notch"),
    ] {
        let json = serde_json::to_string(&kind).expect("serialise");
        assert_eq!(json, format!("\"{spelling}\""));
        let json = format!(r#"{{ "kind": "{spelling}", "cutoff": 1200 }}"#);
        let filter: Filter = serde_json::from_str(&json).expect(spelling);
        assert_eq!(filter.kind, kind);
    }
}

#[test]
fn a_slope_defaults_to_the_single_pole_pair() {
    let filter: Filter =
        serde_json::from_str(r#"{ "kind": "lowpass", "cutoff": 900 }"#).expect("parses");
    assert_eq!(filter.slope, Slope::Db12);
    let steep: Filter =
        serde_json::from_str(r#"{ "kind": "lowpass", "cutoff": 900, "slope": "24db" }"#)
            .expect("parses");
    assert_eq!(steep.slope, Slope::Db24);
}

#[test]
fn a_defaulted_slope_is_not_written_back() {
    let gentle: Filter =
        serde_json::from_str(r#"{ "kind": "lowpass", "cutoff": 900 }"#).expect("parses");
    let saved = serde_json::to_string(&gentle).expect("serialise");
    assert!(!saved.contains("slope"), "slope written back: {saved}");

    let steep = Filter {
        slope: Slope::Db24,
        ..gentle
    };
    let saved = serde_json::to_string(&steep).expect("serialise");
    assert!(saved.contains(r#""slope":"24db""#), "got {saved}");
}
