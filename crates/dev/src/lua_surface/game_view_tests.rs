use super::super::session::Session;
use rusty_core::application::{GameViewSetting, GameViewSize};

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

fn run(s: &mut Session, line: &str) -> String {
    s.command(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

fn windowed() -> Session {
    let s = session();
    let app = &s.world().resources.application;
    app.borrow_mut().mark_game_view_host();
    s
}

fn warned(s: &Session, text: &str) -> bool {
    let console = s.world().console().borrow();
    console.messages.iter().any(|(m, _)| m.contains(text))
}

const READ: &str = "local v = Editor.GetGameViewSize(); \
    return v.mode .. ' ' .. tostring(v.width) .. ' ' .. tostring(v.height) .. ' ' \
    .. tostring(v.low_resolution)";

#[test]
fn sets_read_back_and_reach_the_editor() {
    let mut s = windowed();
    assert_eq!(run(&mut s, READ), "Free nil nil false");
    assert_eq!(run(&mut s, "Editor.SetGameViewSize(1920, 1080)"), "true");
    assert_eq!(run(&mut s, READ), "Fixed 1920 1080 false");
    assert_eq!(run(&mut s, "Editor.SetGameViewAspect(16, 9)"), "true");
    assert_eq!(run(&mut s, "Editor.SetGameViewLowResolution(true)"), "true");
    assert_eq!(run(&mut s, READ), "Aspect 16 9 true");

    let mut editor = GameViewSetting::default();
    let app = &s.world().resources.application;
    app.borrow_mut().sync_game_view(&mut editor);
    assert_eq!(editor.size, GameViewSize::Aspect(16, 9));
    assert!(editor.low_resolution);

    assert_eq!(run(&mut s, "Editor.ResetGameViewSize()"), "true");
    assert_eq!(
        run(&mut s, READ),
        "Free nil nil false",
        "reset is the default"
    );
}

#[test]
fn bad_sizes_are_refused() {
    let mut s = windowed();
    for bad in ["0, 720", "1280, -1", "1280.5, 720", "8193, 10", "0/0, 10"] {
        let line = format!("Editor.SetGameViewSize({bad})");
        assert_eq!(run(&mut s, &line), "false", "{line}");
    }
    assert_eq!(run(&mut s, "Editor.SetGameViewAspect(16, 0)"), "false");
    assert!(warned(&s, "wants two whole numbers from 1 to 8192"));
    assert_eq!(run(&mut s, READ), "Free nil nil false");
}

#[test]
fn a_host_without_a_game_view_refuses() {
    let mut s = session();
    assert_eq!(run(&mut s, "Editor.SetGameViewSize(1280, 720)"), "false");
    assert_eq!(run(&mut s, "Editor.ResetGameViewSize()"), "false");
    assert!(warned(&s, "only the editor window has a Game view to size"));
    assert_eq!(run(&mut s, READ), "Free nil nil false");
}
