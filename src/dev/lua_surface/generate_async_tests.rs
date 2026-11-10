//! `Lighting.GenerateAsync()` refuses where the button does, and where no frame
//! loop would step its job (#1999). The round trip through the editor's job is
//! `editor::inspector::assets::scene::bake`'s `async_tests`.

use super::super::super::session::Session;

/// A session whose host steps a background bake, as the editor window marks itself,
/// with its scene saved under `tag` unless `saved` is false.
fn hosted(tag: &str, saved: bool) -> Session {
    let s = Session::new("").expect("an empty session boots");
    s.world()
        .resources
        .application
        .borrow_mut()
        .mark_bake_host();
    if saved {
        let dir = crate::test_temp::dir().join(format!("generate_async_{tag}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("level.scene").to_string_lossy().into_owned();
        s.world().scene().borrow().save_to_file(&path).unwrap();
        *s.world().script_manager().scene_path_cell().borrow_mut() = Some(path);
    }
    s
}

#[test]
fn a_headless_session_refuses_and_names_the_blocking_verb() {
    let s = Session::new("").expect("an empty session boots");
    let err = s.eval("Lighting.GenerateAsync()").unwrap_err();
    assert!(err.contains("Lighting.Generate()"), "{err}");
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "false");
    assert!(!s.world().scene().borrow().lighting_bake_requested);
}

#[test]
fn an_unsaved_scene_is_refused() {
    let err = hosted("unsaved", false)
        .eval("Lighting.GenerateAsync()")
        .unwrap_err();
    assert!(err.contains("save the scene first"), "{err}");
}

#[test]
fn play_is_refused() {
    let s = hosted("play", true);
    s.world().scene().borrow_mut().set_playing(true);
    let err = s.eval("Lighting.GenerateAsync()").unwrap_err();
    assert!(err.contains(crate::scene::LIGHTING_IN_PLAY), "{err}");
}

#[test]
fn a_running_bake_is_refused_and_reads_as_running() {
    let s = hosted("running", true);
    s.world().scene().borrow_mut().lighting_bake_running = true;
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "true");
    let err = s.eval("Lighting.GenerateAsync()").unwrap_err();
    assert!(err.contains("already running"), "{err}");
}

/// The request runs from the call: a second one, and the blocking verb, wait for it.
#[test]
fn a_request_reads_as_running_and_holds_off_another_bake() {
    let s = hosted("requested", true);
    assert_eq!(s.eval("Lighting.GenerateAsync()").unwrap(), "true");
    assert!(s.world().scene().borrow().lighting_bake_requested);
    assert_eq!(s.eval("Lighting.IsRunning()").unwrap(), "true");
    let again = s.eval("Lighting.GenerateAsync()").unwrap_err();
    assert!(again.contains("already running"), "{again}");
    let blocking = s.eval("Lighting.Generate()").unwrap_err();
    assert!(blocking.contains("already running"), "{blocking}");
}
