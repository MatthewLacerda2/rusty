//! A loader that warms (#2123) holds a prepared operation back until its GPU
//! resources are made, and reports the prepare's steps then the warm-up as progress;
//! a scene with nothing to draw, or a failed one, is finished once prepared.

use super::worker::Threaded;
use crate::app::SceneLoader;
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::Scene;

/// `<tmp>/rusty_2123_warm_<tag>/level.scene`, holding a box when `boxed`.
fn level(tag: &str, boxed: bool) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_2123_warm_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Level".into());
    if boxed {
        scene
            .world
            .set_mesh(id, primitive_mesh_component(Primitive::Box));
    }
    let path = crate::core::paths::to_slash(&dir.join("level.scene"));
    scene.save_to_file(&path).unwrap();
    path
}

/// Ask `loader` until its thread answered operation 1 (`to_warm` or `finished`).
fn prepared(loader: &mut Threaded) {
    for _ in 0..2000 {
        if loader.to_warm().is_some() || loader.finished(1) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the load thread never answered");
}

#[test]
fn a_prepared_scene_is_held_until_it_is_warm() {
    let mut loader = Threaded::warming();
    loader.start(1, level("held", true));
    prepared(&mut loader);
    assert!(!loader.finished(1), "prepared, not warm");
    assert_eq!(loader.progress(1), 0.5, "the prepare's half, none warmed");
    let (data, warming) = loader.to_warm().expect("a box to warm");
    assert_eq!((data.entities.len(), warming.len()), (1, 1));
    let warmed = loader.take(1).expect("a replay may take it cold");
    assert_eq!(warmed.data().entities.len(), 1);
    assert!(loader.to_warm().is_none(), "taken: nothing to warm");
}

#[test]
fn nothing_to_draw_or_a_loader_that_does_not_warm_is_finished_once_prepared() {
    let mut loader = Threaded::warming();
    loader.start(1, level("empty", false));
    prepared(&mut loader);
    assert!(loader.finished(1) && loader.to_warm().is_none());
    assert_eq!(loader.progress(1), 1.0);

    let mut loader = Threaded::default();
    loader.start(1, level("cold", true));
    prepared(&mut loader);
    assert!(loader.finished(1) && loader.to_warm().is_none());
    assert!(loader.progress(1) > 0.5, "the prepare's steps, all of it");
}
