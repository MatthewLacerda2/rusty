use super::*;

/// A per-test temp file, unique to this process so parallel runs never collide.
fn temp_path(name: &str) -> PathBuf {
    let path =
        crate::test_temp::dir().join(format!("rusty_build_{}_{name}.json", std::process::id()));
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
fn saving_the_settings_keeps_the_recorded_engine() {
    let path = temp_path("engine");
    let recorded = ProjectFile {
        engine_commit: "0123abcd".into(),
        ..ProjectFile::default()
    };
    recorded.write(&path).unwrap();
    let mut app = Application::new();
    app.open(&path).unwrap();
    app.update(|b| b.product_name = "Neon".into()).unwrap();
    let file = ProjectFile::read(&path).unwrap().unwrap();
    assert_eq!(file.engine_commit, "0123abcd");
    assert_eq!(file.build.product_name, "Neon");
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
