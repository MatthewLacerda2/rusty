use super::Application;
fn temp_path(name: &str) -> std::path::PathBuf {
    let file = format!("rusty_quality_{}_{name}.json", std::process::id());
    let path = crate::test_temp::dir().join(file);
    let _ = std::fs::remove_file(&path);
    path
}

/// Selecting takes a preset by name in any case and refuses an unknown one,
/// keeping the selection; a removed preset falls back to the initial one.
#[test]
fn selecting_refuses_an_unknown_name_and_removal_falls_back() {
    let mut app = Application::new();
    assert_eq!(app.quality(), Some("Medium"));
    app.select_quality("high").unwrap();
    assert_eq!(app.quality(), Some("High"));
    assert!(app
        .select_quality("Ultra")
        .unwrap_err()
        .contains("Low, Medium, High"));
    assert_eq!(app.quality(), Some("High"));
    app.remove_quality_preset("High").unwrap();
    assert_eq!(app.quality(), Some("Medium"));
    assert!(app.remove_quality_preset("High").is_err());
}

/// The presets round-trip through the bound `project.rusty`; the player's choice
/// never goes into it.
#[test]
fn presets_round_trip_through_the_bound_file() {
    let path = temp_path("presets");
    let mut app = Application::new();
    app.open(&path).unwrap();
    let patch = serde_json::json!({ "ui_blur_divisor": 8 });
    app.edit_quality_preset("Ultra", &patch).unwrap();
    app.remove_quality_preset("Low").unwrap();
    app.select_quality("Ultra").unwrap();
    let mut reopened = Application::new();
    reopened.open(&path).unwrap();
    assert_eq!(reopened.quality_presets(), app.quality_presets());
    assert_eq!(
        reopened.quality_presets().names(),
        ["Medium", "High", "Ultra"]
    );
    assert_eq!(
        reopened.quality_choice(),
        None,
        "the choice is the player's"
    );
    let _ = std::fs::remove_file(&path);
}
