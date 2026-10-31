//! `Scene.GetName` / `Scene.SetName` (#1611): Unity's read/write `gameObject.name`,
//! writing the same `World::set_name` the inspector's name field writes, so a
//! rename is saved with the scene. The nil-id cases are `entity_arg_api`'s shared
//! table.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::scene::Scene;

use crate::play::{playing, run};

#[test]
fn get_name_reads_and_set_name_renames() {
    let mut s = Scene::new();
    let root = s.add_entity("EnemySpawns".to_string());
    let child = s.add_entity("Spawn_TR_1".to_string());
    s.set_parent(child, Some(root)).unwrap();
    let scene = Rc::new(RefCell::new(s));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");

    let first_child = "Scene.GetChildren(Scene.FindEntityByName('EnemySpawns'))[1]";
    assert_eq!(
        eval(&format!("return Scene.GetName({first_child})")),
        "Spawn_TR_1"
    );
    assert_eq!(
        eval(&format!("return Scene.SetName({child}, 'Grunt_12')")),
        "true"
    );
    assert_eq!(
        scene
            .borrow()
            .world
            .name(child)
            .as_deref()
            .map(String::as_str),
        Some("Grunt_12")
    );
    assert_eq!(eval(&format!("return Scene.GetName({child})")), "Grunt_12");
    assert_eq!(
        eval("return Scene.FindEntityByName('Grunt_12') ~= nil"),
        "true",
        "the rename is what FindEntityByName sees"
    );
    // As in Unity: an empty name and a duplicate name are both allowed.
    eval(&format!("Scene.SetName({child}, '')"));
    assert_eq!(
        eval(&format!("return Scene.GetName({child}) == ''")),
        "true"
    );
    eval(&format!("Scene.SetName({child}, 'EnemySpawns')"));
    assert_eq!(
        eval(&format!("return Scene.GetName({child})")),
        "EnemySpawns"
    );
    // A dead id: no rename, and its name reads as nil.
    assert_eq!(eval("return Scene.SetName(99999, 'x')"), "false");
    assert_eq!(eval("return Scene.GetName(99999) == nil"), "true");
}

#[test]
fn a_rename_is_saved_with_the_scene() {
    let mut s = Scene::new();
    let id = s.add_entity("Crate".to_string());
    let scene = Rc::new(RefCell::new(s));
    let game = playing(scene.clone(), 1.0);
    game.script_manager()
        .eval(&format!("Scene.SetName({id}, 'Crate_3')"))
        .expect("evaluates");
    let dir = crate::temp::dir();
    let path = dir
        .join("rusty_scene_name.scene")
        .to_string_lossy()
        .into_owned();
    scene.borrow().save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.find_entity_by_name("Crate_3"), Some(id));
}
