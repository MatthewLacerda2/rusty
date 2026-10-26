//! The volume's stored shape (#1467): one object per effect, each defaulting when a
//! file leaves it, or any of its fields, out.

use super::*;

fn load(json: &str) -> PostProcessingComponent {
    serde_json::from_str(json).unwrap()
}

fn round_trip(vc: &PostProcessingComponent) -> PostProcessingComponent {
    serde_json::from_str(&serde_json::to_string(vc).unwrap()).unwrap()
}

#[test]
fn an_empty_volume_loads_active_with_every_effect_at_its_default() {
    let vc = load("{}");
    assert!(vc.active, "a volume applies unless it says otherwise");
    assert_eq!(
        vc,
        PostProcessingComponent::default(),
        "`{{}}` is the default"
    );
}

#[test]
fn an_effect_object_keeps_the_defaults_of_the_fields_it_leaves_out() {
    let vc = load(r#"{"depth_of_field": {"mode": "Bokeh", "aperture": 1.4}}"#);
    let dof = round_trip(&vc).depth_of_field;
    assert_eq!((dof.mode, dof.aperture), (DofMode::Bokeh, 1.4));
    assert_eq!(dof.focus_distance, 10.0, "an absent knob keeps its default");
    let vc = load(r#"{"exposure": {"mode": "Automatic", "automatic": {"max_ev": 3.0}}}"#);
    let back = round_trip(&vc).exposure;
    assert_eq!(back.mode, ExposureMode::Automatic);
    assert_eq!((back.automatic.max_ev, back.automatic.min_ev), (3.0, -6.0));
    let vc = load(r#"{"shadows": {"cascades": 2}, "ssao": {"radius": 1.5}}"#);
    assert_eq!((vc.shadows.cascades, vc.shadows.distance), (2, 100.0));
    assert_eq!((vc.ssao.active, vc.ssao.radius), (true, 1.5));
}

#[test]
fn the_grade_loads_neutral_and_round_trips() {
    let vc = load("{}");
    assert!(vc.white_balance.is_neutral() && vc.color_adjustments.hue_shift == 0.0);
    assert_eq!(vc.color_lookup, ColorLookup::default());
    assert_eq!(vc.tonemapping.mode, Tonemap::Aces);
    let vc = load(
        r#"{"tonemapping": {"display_gamma": 1.3},
        "lift_gamma_gain": {"gain": {"offset": 0.5}}}"#,
    );
    let back = round_trip(&vc);
    assert_eq!(back.tonemapping.display_gamma, 1.3);
    assert_eq!(
        back.lift_gamma_gain.gain.color, [1.0; 3],
        "an absent colour is white"
    );
    assert_eq!(back.lift_gamma_gain.gain.offset, 0.5);
}

#[test]
fn custom_effects_keep_their_order_and_params() {
    assert_eq!(load("{}").custom_effects, CustomEffects::default());
    let vc = load(
        r#"{"custom_effects": {"names": ["crt", "vignette_red"],
        "params": {"damage_vignette.intensity": [0.7]}}}"#,
    );
    let back = round_trip(&vc).custom_effects;
    assert_eq!(back.names, ["crt", "vignette_red"]);
    assert_eq!(back.params["damage_vignette.intensity"], [0.7]);
}

#[test]
fn every_field_but_the_listed_settings_is_an_effect_object() {
    let json = serde_json::to_value(PostProcessingComponent::default()).unwrap();
    for (name, value) in json.as_object().unwrap() {
        let effect = !PostProcessingComponent::NOT_EFFECTS.contains(&name.as_str());
        assert_eq!(
            effect,
            value.is_object() && name != "shadows" && name != "ssao",
            "{name}"
        );
    }
}
