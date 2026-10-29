//! The quality presets and `project.rusty` (#1522): the editor re-reads the presets
//! from the bound file at each Play, so a hand edit runs, and a save of something
//! else writes back the file's presets, never a copy from before the edit.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::core::project::ProjectFile;
use rusty::scene::Scene;

#[test]
fn a_hand_edit_to_the_presets_runs_at_play_and_survives_a_save() {
    let path = crate::temp::file("project.rusty");
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = crate::play::playing(scene, 20.0);
    game.set_playing(false);
    let app = Rc::clone(&game.resources.application);
    app.borrow_mut().open(&path).unwrap();
    app.borrow_mut().update(|_| {}).unwrap(); // the file as the editor first writes it

    // By hand: the running (`Medium`) preset's `Medium` SSAO level takes 12 samples.
    let mut file = ProjectFile::read(&path).unwrap().unwrap();
    let medium = file.quality.presets.iter_mut().find(|p| p.name == "Medium");
    medium.unwrap().values.ssao.medium.samples = 12;
    file.write(&path).unwrap();

    game.set_playing(true);
    game.tick(1.0 / 60.0); // enter Play
    let graphics = game.script_manager().graphics_cell();
    assert_eq!(graphics.borrow().quality.ssao.medium.samples, 12);

    app.borrow_mut()
        .update(|build| build.product_name = "Edited".into())
        .unwrap();
    let saved = ProjectFile::read(&path).unwrap().unwrap();
    assert_eq!(saved.build.product_name, "Edited");
    let medium = saved.quality.get("Medium").unwrap();
    assert_eq!(
        medium.values.ssao.medium.samples, 12,
        "the save kept the edit"
    );
}

#[test]
fn a_save_never_writes_back_presets_from_before_a_hand_edit() {
    let path = crate::temp::file("project.rusty");
    let mut app = rusty::core::application::Application::new();
    app.open(&path).unwrap();
    app.update(|_| {}).unwrap();
    let mut file = ProjectFile::read(&path).unwrap().unwrap();
    file.quality.presets.retain(|p| p.name != "Low");
    file.write(&path).unwrap();
    // No Play in between: the save itself re-reads.
    app.set_default_quality(Some("High")).unwrap();
    let saved = ProjectFile::read(&path).unwrap().unwrap();
    assert_eq!(saved.quality.names(), ["Medium", "High"]);
    assert_eq!(saved.quality.default.as_deref(), Some("High"));
}
