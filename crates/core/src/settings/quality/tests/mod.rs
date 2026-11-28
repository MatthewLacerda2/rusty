use super::{EffectQuality, QualityPreset, QualityPresets, QualityValues};

#[test]
fn a_new_project_has_the_three_seeded_presets() {
    let presets = QualityPresets::default();
    assert_eq!(presets.names(), ["Low", "Medium", "High"]);
    assert_eq!(presets.initial().map(|p| p.name.as_str()), Some("Medium"));
}

#[test]
fn seeded_presets_hold_the_old_tier_numbers() {
    let presets = QualityPresets::default();
    let ssao = |name| presets.get(name).unwrap().values.ssao.medium;
    assert_eq!(
        (ssao("Medium").samples, ssao("Medium").resolution_divisor),
        (8, 2)
    );
    assert_eq!(
        (ssao("High").samples, ssao("High").resolution_divisor),
        (16, 1)
    );
}

#[test]
fn a_level_picks_its_row_and_custom_the_authored_value() {
    let bloom = QualityValues::default().bloom_resolution_divisor;
    assert_eq!(EffectQuality::Low.pick(&bloom, 7), bloom.low);
    assert_eq!(EffectQuality::High.pick(&bloom, 7), bloom.high);
    assert_eq!(EffectQuality::Custom.pick(&bloom, 7), 7);
}

#[test]
fn resolve_falls_back_to_the_initial_preset() {
    let mut presets = QualityPresets {
        default: None,
        ..Default::default()
    };
    assert_eq!(presets.resolve(None).unwrap().name, "Medium");
    assert_eq!(presets.resolve(Some("high")).unwrap().name, "High");
    assert_eq!(presets.resolve(Some("Gone")).unwrap().name, "Medium");
    presets.presets.clear();
    assert!(presets.resolve(None).is_none());
    assert_eq!(presets.values(None), QualityValues::default());
}

#[test]
fn set_replaces_or_appends_and_remove_reports() {
    let mut presets = QualityPresets::default();
    let mut ultra = presets.get("High").unwrap().clone();
    ultra.name = "Ultra".to_string();
    presets.set(ultra.clone());
    ultra.values.ui_blur_divisor = 8;
    presets.set(QualityPreset {
        name: "ultra".to_string(),
        ..ultra
    });
    assert_eq!(presets.names(), ["Low", "Medium", "High", "ultra"]);
    assert!(presets.remove("ULTRA"));
    assert!(!presets.remove("Ultra"));
    assert!(presets.unknown("Ultra").contains("Low, Medium, High"));
}

#[test]
fn presets_round_trip_through_json() {
    let presets = QualityPresets::default();
    let text = serde_json::to_string(&presets).unwrap();
    assert_eq!(
        serde_json::from_str::<QualityPresets>(&text).unwrap(),
        presets
    );
}

#[test]
fn an_edit_writes_only_what_it_names() {
    let presets = QualityPresets::default();
    let patch = serde_json::json!({ "ssao": { "high": { "samples": 48 } } });
    let edited = presets.edited("High", &patch).unwrap();
    let high = presets.get("High").unwrap().values;
    assert_eq!(edited.values.ssao.high.samples, 48);
    assert_eq!(edited.values.ssao.medium, high.ssao.medium);
    let typo = serde_json::json!({ "ssao": { "hihg": {} } });
    assert!(presets
        .edited("High", &typo)
        .unwrap_err()
        .contains("ssao.hihg"));
    let fresh = presets.edited("Ultra", &serde_json::json!({})).unwrap();
    assert_eq!(fresh.values, QualityValues::default());
}

#[test]
fn a_partial_preset_loads_over_the_seeded_medium_values() {
    let text = r#"{ "presets": [
        { "name": "Low" },
        { "name": "Ultra", "ssao": { "high": { "samples": 48 } } },
        { "name": "High" } ] }"#;
    let presets: QualityPresets = serde_json::from_str(text).unwrap();
    assert_eq!(presets.default, None);
    assert_eq!(presets.initial().unwrap().name, "Ultra", "the middle one");
    let ultra = presets.get("Ultra").unwrap().values;
    let medium = QualityValues::default();
    assert_eq!(ultra.ssao.high.samples, 48);
    assert_eq!(
        ultra.ssao.high.resolution_divisor,
        medium.ssao.high.resolution_divisor
    );
    assert_eq!(ultra.ssao.low, medium.ssao.low);
    assert_eq!(ultra.ssr, medium.ssr);
    let typo = r#"{ "presets": [ { "name": "X", "ssao": { "hihg": {} } } ] }"#;
    let err = serde_json::from_str::<QualityPresets>(typo).unwrap_err();
    assert!(err.to_string().contains("ssao.hihg"), "{err}");
}

#[test]
fn a_preset_naming_only_a_budget_keeps_streaming_on() {
    let text =
        r#"{ "presets": [ { "name": "Tight", "texture_streaming": { "budget_mb": 64 } } ] }"#;
    let presets: QualityPresets = serde_json::from_str(text).unwrap();
    let tight = presets.get("Tight").unwrap().values.texture_streaming;
    assert_eq!((tight.active, tight.budget_mb), (true, 64));
}

mod seed_rows;
