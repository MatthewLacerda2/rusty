//! `Camera.ResetHistory([id])` (#2003): a script cuts the main camera's view, or a
//! named camera's, and a bare entity or a dead id changes nothing.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::scene::authoring::default_camera;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

#[test]
fn a_script_cuts_the_main_camera_or_the_one_it_names() {
    let mut scene = Scene::new();
    let main = scene.add_entity("MainCamera".to_string());
    scene.world.set_camera(main, Some(default_camera()));
    let monitor = scene.add_entity("Monitor".to_string());
    scene.world.set_camera(monitor, Some(default_camera()));
    let bare = scene.add_entity("Bare".to_string());
    let driver = scene.add_entity("Driver".to_string());
    let script = format!(
        "local done = false\n\
         return {{ Update = function(id)\n\
         if done then return end\n\
         done = true\n\
         Camera.ResetHistory()\n\
         Camera.ResetHistory(nil)\n\
         Camera.ResetHistory({monitor})\n\
         Camera.ResetHistory({bare})\n\
         Camera.ResetHistory(99999)\nend }}"
    );
    attach_script(&mut scene, driver, "reset_history.lua", &script);
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(Rc::clone(&scene), 10.0);
    run(&mut game, 2);
    let s = scene.borrow();
    let cuts = |id| s.history_cuts.of(id);
    assert_eq!(cuts(Some(main)), 2, "no id and nil cut the main camera");
    assert_eq!(cuts(Some(monitor)), 1, "a named camera");
    assert_eq!(cuts(Some(bare)), 0, "an entity with no camera");
    assert_eq!(cuts(None), 0, "the free camera, with a main camera present");
}
