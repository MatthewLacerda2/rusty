//! The Scene view's size the round reports (`editor.view_w`, `editor.view_h`):
//! the size the view was really drawn at, which the panels beside it set.

use super::editor::Editor;
use super::tests::{project, SIZE};
use super::*;

/// The Scene view's size in one measured frame of `editor`.
fn view(editor: &mut Editor, game: &mut GameWorld) -> (u32, u32) {
    let (_, drawn, _, _) = editor.frame(game);
    drawn.expect("the Scene view has room at SIZE").view
}

#[test]
fn gpu_the_scene_view_is_wider_with_the_hierarchy_closed() {
    let (_root, scene) = project();
    let opts = Round {
        size: SIZE,
        warmup: 0,
        frames: 1,
        runs: 1,
        warm: false,
        switches: rusty_render::RenderSwitches::ON,
        save_dir: rusty_test_support::temp::dir().join("editor_view_unused"),
    };
    let mut game = editor_world(&scene, ProjectSettings::default()).unwrap();
    let Some(mut editor) = Editor::new(&game, &opts) else {
        return;
    };
    let open = view(&mut editor, &mut game);
    assert!(
        open.0 < SIZE.0 && open.1 <= SIZE.1,
        "inside the window: {open:?}"
    );
    editor.editor_ui().hierarchy_open = false;
    view(&mut editor, &mut game);
    let closed = view(&mut editor, &mut game);
    assert!(closed.0 > open.0, "{closed:?} against {open:?}");
    assert_eq!(closed.1, open.1, "the panels sit beside the view");
}
