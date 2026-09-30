use super::*;

/// A per-test temp file, unique to this process so parallel runs never collide.
fn temp_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("rusty_build_{}_{name}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn default_startup_scene_is_the_seeded_scene() {
    assert_eq!(DEFAULT_STARTUP_SCENE, crate::scene::DEFAULT_SCENE_PATH);
    assert_eq!(
        BuildSettings::default().startup_scene,
        DEFAULT_STARTUP_SCENE
    );
}

#[test]
fn partial_file_keeps_defaults_for_missing_fields() {
    let b = BuildSettings::from_json(r#"{ "product_name": "Neon" }"#).unwrap();
    assert_eq!(b.product_name, "Neon");
    assert_eq!(b.startup_scene, DEFAULT_STARTUP_SCENE);
    assert_eq!(b.window_mode, WindowMode::Windowed);
}

#[test]
fn malformed_file_is_an_error() {
    assert!(BuildSettings::from_json("{ not json").is_err());
}

#[test]
fn json_roundtrips() {
    let b = BuildSettings {
        startup_scene: "project/scenes/menu.scene".into(),
        product_name: "Neon".into(),
        window_mode: WindowMode::Fullscreen,
    };
    assert_eq!(BuildSettings::from_json(&b.to_json()).unwrap(), b);
}

#[test]
fn missing_file_reads_as_defaults() {
    let path = temp_path("absent");
    assert_eq!(
        BuildSettings::read(&path).unwrap(),
        BuildSettings::default()
    );
}

#[test]
fn window_mode_names_parse_case_insensitively() {
    assert_eq!(
        WindowMode::parse("FULLSCREEN"),
        Some(WindowMode::Fullscreen)
    );
    assert_eq!(WindowMode::parse("windowed"), Some(WindowMode::Windowed));
    assert_eq!(WindowMode::parse("borderless"), None);
    for mode in [WindowMode::Windowed, WindowMode::Fullscreen] {
        assert_eq!(WindowMode::parse(mode.name()), Some(mode));
    }
}

#[test]
fn bound_instance_writes_through_on_every_setter() {
    let path = temp_path("bound");
    let mut app = Application::new();
    app.open(&path).unwrap();
    app.update(|b| b.product_name = "Neon".into()).unwrap();
    assert_eq!(BuildSettings::read(&path).unwrap().product_name, "Neon");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn loaded_instance_never_writes_the_file() {
    let path = temp_path("loaded");
    std::fs::write(&path, BuildSettings::default().to_json()).unwrap();
    let mut app = Application::new();
    app.load(&path).unwrap();
    app.update(|b| b.product_name = "Changed".into()).unwrap();
    assert_eq!(app.build().product_name, "Changed");
    assert_eq!(
        BuildSettings::read(&path).unwrap().product_name,
        DEFAULT_PRODUCT_NAME
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn quit_request_is_taken_exactly_once() {
    let mut app = Application::new();
    assert!(!app.take_quit_request());
    app.request_quit();
    assert!(app.quit_requested());
    assert!(app.take_quit_request());
    assert!(!app.quit_requested());
    assert!(!app.take_quit_request());
}
