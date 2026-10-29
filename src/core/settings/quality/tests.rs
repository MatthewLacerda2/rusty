use super::{EffectQuality, QualityPreset, QualityPresets, QualityValues};

/// A new project is seeded with `Low`, `Medium`, `High`, and selects `Medium`.
#[test]
fn a_new_project_has_the_three_seeded_presets() {
    let presets = QualityPresets::default();
    assert_eq!(presets.names(), ["Low", "Medium", "High"]);
    assert_eq!(presets.initial().map(|p| p.name.as_str()), Some("Medium"));
}

/// The seeded `Medium` preset's `Medium` level keeps the old Medium tier's SSAO
/// (8 samples at half resolution), and `High`'s the old High tier's (16 at full).
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

/// A level takes the preset's row; `Custom` takes the authored value.
#[test]
fn a_level_picks_its_row_and_custom_the_authored_value() {
    let bloom = QualityValues::default().bloom_resolution_divisor;
    assert_eq!(EffectQuality::Low.pick(&bloom, 7), bloom.low);
    assert_eq!(EffectQuality::High.pick(&bloom, 7), bloom.high);
    assert_eq!(EffectQuality::Custom.pick(&bloom, 7), 7);
}

/// The middle preset runs when no `default` names one; a removed selection falls
/// back to the initial preset; no presets at all run the engine's own values.
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

/// `set` replaces a preset of the same name in place and appends a new one;
/// `remove` reports whether there was one.
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

/// The presets round-trip through JSON, as `project.rusty` stores them.
#[test]
fn presets_round_trip_through_json() {
    let presets = QualityPresets::default();
    let text = serde_json::to_string(&presets).unwrap();
    assert_eq!(
        serde_json::from_str::<QualityPresets>(&text).unwrap(),
        presets
    );
}

/// A partial edit keeps every value it doesn't name; an unknown key is refused.
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
