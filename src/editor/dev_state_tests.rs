use super::*;

#[test]
fn console_is_hidden_by_default() {
    let mut state = DevEditorState::default();
    assert!(!state.show_play_console);
    state.load_prefs(&Storage::new());
    assert!(!state.show_play_console);
}

#[test]
fn toggle_persists_through_storage() {
    let mut storage = Storage::new();
    let mut state = DevEditorState::default();
    assert!(!state.store_prefs(&mut storage), "unchanged writes nothing");
    state.show_play_console = true;
    assert!(state.store_prefs(&mut storage));
    assert!(!state.store_prefs(&mut storage), "stored once");

    let mut reopened = DevEditorState::default();
    reopened.load_prefs(&storage);
    assert!(reopened.show_play_console);

    state.show_play_console = false;
    assert!(state.store_prefs(&mut storage));
    reopened.load_prefs(&storage);
    assert!(!reopened.show_play_console);
}
