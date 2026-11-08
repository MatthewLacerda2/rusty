use super::*;

#[test]
fn console_is_hidden_by_default() {
    assert!(!DevEditorState::default().show_play_console);
}
