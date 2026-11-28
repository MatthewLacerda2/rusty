//! Additive scenes and determinism: two additive loads then an unload, recorded with the
//! loader finishing late, replay bit for bit on a loader that finishes at another moment and
//! on a headless host's, which prepares inline.

use rusty::app::Inline;
use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::scene::Scene;

use super::load_async::{eval, Late};
use super::replay;

/// Two pieces beside the demo, each a box under a root.
fn pieces(dir: &std::path::Path) -> [String; 2] {
    std::fs::create_dir_all(dir).unwrap();
    ["a", "b"].map(|name| {
        let path = rusty::core::paths::to_slash(&dir.join(format!("{name}.scene")));
        let mut scene = Scene::new();
        let root = scene.add_entity(format!("Piece_{name}"));
        let part = rusty::scene::create::create_entity(
            &mut scene,
            "Part",
            Some(rusty::scene::create::Primitive::Box),
        );
        scene.set_parent(part, Some(root)).unwrap();
        rusty::edit::save_to_file(&scene, &path).unwrap();
        path
    })
}

#[test]
fn two_additive_loads_and_an_unload_replay_on_any_loader() {
    let dir = crate::temp::dir().join("demo_additive_replay");
    let [a, b] = pieces(&dir);
    let path = dir.join("run.demo");
    let mut h = Harness::new(dir.join("record"), "");
    demo::record(&mut h.world.borrow_mut(), Destination::File(path.clone()));
    h.world
        .borrow_mut()
        .set_scene_loader(Box::new(Late::new(3)));
    h.step(10);
    eval(&h, &format!("first = Scene.LoadAsync('{a}', 'Additive')"));
    eval(&h, &format!("second = Scene.LoadAsync('{b}', 'Additive')"));
    h.step(40);
    assert_eq!(eval(&h, "return Scene.GetLoadStatus(second)"), "done");
    assert_eq!(eval(&h, "return #Scene.GetLoaded()"), "3");
    eval(&h, &format!("Scene.Unload('{a}')"));
    h.step(100);
    assert_eq!(eval(&h, "return #Scene.GetLoaded()"), "2");
    demo::conclude(&mut h);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    let ready: Vec<Vec<u32>> = demo
        .ticks()
        .map(|t| t.loads)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        ready,
        [vec![1], vec![2]],
        "one after the other, in request order"
    );
    for after in [0, 25] {
        let verdict = replay(&dir, Demo::read(&path).unwrap(), |h| {
            h.world
                .borrow_mut()
                .set_scene_loader(Box::new(Late::new(after)));
        });
        assert_eq!(
            verdict.diverged, None,
            "loader done after {after} asks: {verdict:?}"
        );
        assert!(verdict.checked >= 2, "{verdict:?}");
    }
    let headless = replay(&dir, Demo::read(&path).unwrap(), |h| {
        h.world
            .borrow_mut()
            .set_scene_loader(Box::new(Inline::default()));
    });
    assert_eq!(headless.diverged, None, "{headless:?}");
}
