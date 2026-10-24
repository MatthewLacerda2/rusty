//! The editor's current scene file is the script runtime's own cell (#1366): what
//! File ▸ Open/New and `Scene.Load` set is what the next Save writes.

use std::cell::RefCell;
use std::rc::Rc;

use super::{new_scene, save_scene};
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Scene files `a`, `b` and `c` (one entity each, named after the file) in a fresh
/// dir, and a world plus editor with `a` open, sharing the cell as the shell does.
fn open_a(tag: &str) -> ([String; 3], GameWorld, EditorUi) {
    let dir = crate::test_temp::dir().join(format!("scene_path_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let paths = ["a", "b", "c"].map(|name| {
        let path = dir
            .join(format!("{name}.scene"))
            .to_string_lossy()
            .into_owned();
        let mut scene = Scene::new();
        scene.add_entity(name.to_string());
        scene.save_to_file(&path).unwrap();
        path
    });
    let mut scene = Scene::new();
    scene.load_from_file(&paths[0]).unwrap();
    let game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -8.0, 8.0, -8.0, 8.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    let mut editor = EditorUi::new();
    editor.share_scene_path(game.script_manager().scene_path_cell());
    editor.set_current_scene_path(Some(paths[0].clone()));
    (paths, game, editor)
}

fn script_path(game: &GameWorld) -> Option<String> {
    game.script_manager().scene_path_cell().borrow().clone()
}

#[test]
fn open_b_then_save_writes_b_and_leaves_a() {
    let ([a, b, _], game, mut editor) = open_a("open");
    let a_before = std::fs::read(&a).unwrap();
    let mut console = ConsoleLogs::new();
    let mut scene = game.scene().borrow_mut();
    editor.open_scene(&mut scene, &mut console, &b, false);
    assert_eq!(
        script_path(&game),
        Some(b.clone()),
        "Scene.Save() targets B"
    );
    scene.add_entity("Edit".into());
    save_scene(&mut editor, &scene, &mut console);
    assert_eq!(std::fs::read(&a).unwrap(), a_before, "A is untouched");
    let b_text = std::fs::read_to_string(&b).unwrap();
    assert!(b_text.contains("\"Edit\""), "the save went to B");
}

#[test]
fn new_leaves_no_path_for_either_caller() {
    let (_, game, mut editor) = open_a("new");
    let mut console = ConsoleLogs::new();
    new_scene(
        &mut editor,
        &mut game.scene().borrow_mut(),
        &mut console,
        false,
    );
    assert_eq!(editor.current_scene_path(), None);
    assert_eq!(script_path(&game), None);
}

#[test]
fn a_scripted_load_is_what_the_editor_saves() {
    let ([_, _, c], mut game, mut editor) = open_a("script");
    game.init_edit_runtime().unwrap();
    let load = format!("Scene.Load('{}')", c.replace('\\', "/"));
    game.script_manager().eval(&load).unwrap();
    assert_eq!(editor.current_scene_path(), Some(c.clone()));
    let mut scene = game.scene().borrow_mut();
    scene.add_entity("Edit".into());
    save_scene(&mut editor, &scene, &mut ConsoleLogs::new());
    assert!(std::fs::read_to_string(&c).unwrap().contains("\"Edit\""));
}
