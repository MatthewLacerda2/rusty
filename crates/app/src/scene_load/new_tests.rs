//! `Scene.New()` (#2000), the API face of File ▸ New: in edit mode the World becomes
//! an empty, new scene with no current file; during Play it is refused and nothing
//! changes.

use super::fixture::{world_at, write_scenes};

const DT: f32 = 1.0 / 60.0;

#[test]
fn edit_mode_new_empties_the_world_and_clears_the_file() {
    let (menu, _) = write_scenes("new_edit");
    let (mut gw, _) = world_at(&menu);
    gw.init_edit_runtime().unwrap();
    let before = gw.scene().borrow().id();
    assert!(gw.scene().borrow().entity_count() > 0);

    gw.script_manager().eval("Scene.New()").unwrap();
    let scene = gw.scene().borrow();
    assert_eq!(scene.entity_count(), 0, "an empty scene");
    assert_ne!(scene.id(), before, "a new scene: an open editor follows it");
    drop(scene);
    assert_eq!(*gw.script_manager().scene_path_cell().borrow(), None);
    let active = gw.script_manager().eval("return Scene.GetActivePath()");
    assert_eq!(active.unwrap(), "nil");
    let save = gw.script_manager().eval("Scene.Save()").unwrap_err();
    assert!(save.contains("no current scene file"), "{save}");
    let before_menu = std::fs::read_to_string(&menu).unwrap();
    assert!(before_menu.contains("Menu"), "the old file is untouched");
}

#[test]
fn new_is_refused_in_play() {
    let (menu, _) = write_scenes("new_play");
    let (mut gw, _) = world_at(&menu);
    gw.set_playing(true);
    gw.tick(DT);
    let path = gw.script_manager().scene_path_cell().borrow().clone();
    let (id, count) = {
        let scene = gw.scene().borrow();
        (scene.id(), scene.entity_count())
    };
    let err = gw.script_manager().eval("Scene.New()").unwrap_err();
    assert!(err.contains("Scene.New") && err.contains("Play"), "{err}");
    let scene = gw.scene().borrow();
    assert_eq!((scene.id(), scene.entity_count()), (id, count));
    drop(scene);
    assert!(path.is_some());
    assert_eq!(*gw.script_manager().scene_path_cell().borrow(), path);
}
