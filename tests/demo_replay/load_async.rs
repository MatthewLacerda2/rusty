//! `Scene.LoadAsync` and determinism (#2123): an async load leaves the world a sync
//! load of the same file on the same tick leaves, and a demo recorded with the loader
//! finishing late replays bit for bit on a loader that finishes at another moment.

use rusty::app::{Inline, SceneLoader};
use rusty::dev::demo::{self, state_digest, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::scene::runtime::prepare::PreparedScene;
use rusty::scene::Scene;

use super::replay;

/// The default scene, saved as `<dir>/level.scene` with no navmesh beside it, so
/// the load bakes one (off the tick for `LoadAsync`).
fn level(dir: &std::path::Path) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let path = rusty::core::paths::to_slash(&dir.join("level.scene"));
    let mut scene = Scene::new();
    rusty::scene::default_scene::build(&mut scene, "");
    scene.save_to_file(&path).unwrap();
    path
}

/// A loader that reports a job finished only `after` asks: a worker still running.
struct Late {
    inner: Inline,
    after: u32,
    asked: u32,
}

impl Late {
    fn new(after: u32) -> Self {
        Self {
            inner: Inline::default(),
            after,
            asked: 0,
        }
    }
}

impl SceneLoader for Late {
    fn start(&mut self, op: u32, path: String) {
        self.asked = 0;
        self.inner.start(op, path);
    }
    fn finished(&mut self, op: u32) -> bool {
        self.asked += 1;
        self.asked > self.after && self.inner.finished(op)
    }
    fn take(&mut self, op: u32) -> Result<PreparedScene, String> {
        self.inner.take(op)
    }
    fn cancel(&mut self) {
        self.inner.cancel();
    }
}

fn eval(h: &Harness, code: &str) -> String {
    h.world.borrow().script_manager().eval(code).unwrap()
}

/// Step once into Play, call `verb(level)`, step 30 ticks: the active path after the
/// first of them, and the digest after the last.
fn load_with(dir: &std::path::Path, verb: &str, level: &str) -> (String, String) {
    let h = Harness::new(dir.join(verb), "");
    h.step(1);
    eval(&h, &format!("{verb}('{level}')"));
    h.step(1);
    let path = eval(&h, "return Scene.GetActivePath()");
    h.step(29);
    let digest = state_digest(&h.world.borrow());
    (path, digest)
}

#[test]
fn an_async_load_leaves_the_world_a_sync_load_of_that_tick_leaves() {
    let dir = crate::temp::dir().join("demo_load_async_digest");
    let level = level(&dir);
    let sync = load_with(&dir, "Scene.Load", &level);
    let ready = load_with(&dir, "Scene.LoadAsync", &level);
    assert_eq!(sync.0, level, "the sync load swapped on its tick");
    assert_eq!(ready, sync, "same tick, same world");
}

#[test]
fn a_demo_across_an_async_load_replays_with_the_worker_at_another_moment() {
    let dir = crate::temp::dir().join("demo_load_async_replay");
    let level = level(&dir);
    let path = dir.join("run.demo");
    let mut h = Harness::new(dir.join("record"), "");
    demo::record(&mut h.world.borrow_mut(), Destination::File(path.clone()));
    h.world
        .borrow_mut()
        .set_scene_loader(Box::new(Late::new(6)));
    h.step(10);
    eval(&h, &format!("op = Scene.LoadAsync('{level}')"));
    h.step(130);
    assert_eq!(eval(&h, "return Scene.GetLoadStatus(op)"), "done");
    demo::conclude(&mut h);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    let ready: Vec<(u64, Vec<u32>)> = demo
        .ticks()
        .filter(|t| !t.loads.is_empty())
        .map(|t| (t.t, t.loads))
        .collect();
    assert_eq!(
        ready,
        [(16, vec![1])],
        "ready on the seventh head after the call"
    );
    for after in [0, 40] {
        let demo = Demo::read(&path).unwrap();
        let verdict = replay(&dir, demo, |h| {
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
}
