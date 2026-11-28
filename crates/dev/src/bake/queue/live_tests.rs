use super::{live_files, refuse, UNSAVED};
use rusty_scene::Scene;

/// `base.scene` opened, `level.scene` with its door loaded beside it.
fn two_loaded() -> (Scene, String, String) {
    let dir = rusty_test_support::temp::dir();
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let (base, level) = (path("base.scene"), path("level.scene"));
    let mut one = Scene::new();
    one.add_entity("Player".into());
    rusty_edit::save_to_file(&one, &base).unwrap();
    let mut other = Scene::new();
    other.add_entity("Door".into());
    rusty_edit::save_to_file(&other, &level).unwrap();
    let mut scene = Scene::new();
    rusty_edit::load_from_file(&mut scene, &base).unwrap();
    rusty_edit::scenes::load_additive(&mut scene, Some(base.clone()), &level).unwrap();
    (scene, base, level)
}

#[test]
fn an_unsaved_edit_to_any_loaded_scene_refuses_the_queue() {
    let (mut scene, base, level) = two_loaded();
    assert_eq!(refuse(&scene, Some(&base), false), Ok(()));
    assert_eq!(live_files(&scene, Some(&base)), [base.clone(), level]);
    let door = scene.find_entity_by_name("Door").unwrap();
    scene.world.set_name(door, "Gate".into());
    let refused = refuse(&scene, Some(&base), false);
    assert_eq!(
        refused,
        Err(UNSAVED.to_string()),
        "the level, not the active base"
    );
}
