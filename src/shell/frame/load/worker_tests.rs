//! The windowed loader prepares on a thread (#2123): a job is taken once, by its own
//! operation, waiting when the thread is still at it; a bad file fails the job.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::Threaded;
use crate::app::SceneLoader;
use crate::core::paths;
use crate::scene::Scene;

fn scene_file(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_2123_worker_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = paths::to_slash(&dir.join("level.scene"));
    let mut scene = Scene::new();
    scene.add_entity("Level".into());
    scene.save_to_file(&path).unwrap();
    path
}

#[test]
fn a_job_is_prepared_off_the_thread_and_taken_by_its_operation() {
    let path = scene_file("take");
    let mut loader = Threaded::default();
    loader.start(3, path.clone());
    assert!(
        !loader.finished(2),
        "another operation's job is not this one"
    );
    assert!(loader.take(2).is_err(), "nor can another operation take it");
    loader.start(4, path.clone());
    let prepared = loader
        .take(4)
        .expect("waits for the thread, then hands it over");
    assert_eq!(prepared.path(), path);
    assert!(!loader.finished(4), "a job is taken once");
}

#[test]
fn a_file_that_does_not_parse_fails_its_job() {
    let path = scene_file("bad");
    std::fs::write(&path, "{ not a scene").unwrap();
    let mut loader = Threaded::default();
    loader.start(1, path);
    assert!(loader.take(1).is_err());
}

#[test]
fn a_cancelled_job_is_gone() {
    let mut loader = Threaded::default();
    loader.start(1, scene_file("cancel"));
    loader.cancel();
    assert!(!loader.finished(1));
    assert!(loader.take(1).is_err());
}

#[test]
fn a_replaced_or_cancelled_job_is_abandoned() {
    let mut loader = Threaded::default();
    let flag = |l: &Threaded| l.job.as_ref().unwrap().abandoned.clone();
    loader.start(1, scene_file("replace"));
    let first = flag(&loader);
    loader.start(2, scene_file("replace"));
    assert!(
        first.load(Ordering::Relaxed),
        "a replaced job is told to stop"
    );
    let second = flag(&loader);
    assert!(!second.load(Ordering::Relaxed));
    loader.cancel();
    assert!(second.load(Ordering::Relaxed), "so is a cancelled one");
}

#[test]
fn an_abandoned_job_ends_without_its_bake() {
    // No navmesh beside it: a job left to run would bake one.
    let path = scene_file("abandoned");
    let (rx, thread) = super::spawn(path, Arc::new(AtomicBool::new(true)), Default::default());
    thread.unwrap().join().expect("the thread ends");
    match rx.recv().unwrap() {
        Err(why) => assert_eq!(why, crate::app::ABANDONED),
        Ok(_) => panic!("an abandoned job prepared its scene"),
    }
}

#[test]
fn the_shell_reaches_its_installed_loader() {
    use std::{cell::RefCell, rc::Rc};
    let mut game = crate::app::GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(crate::core::input::InputState::new())),
        Rc::new(RefCell::new(crate::navigation::NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(crate::scripting::ConsoleLogs::new())),
    );
    game.set_scene_loader(Box::new(Threaded::default()));
    assert!(game.scene_loader_mut::<Threaded>().is_some());
    assert!(game.scene_loader_mut::<crate::app::Inline>().is_none());
}
