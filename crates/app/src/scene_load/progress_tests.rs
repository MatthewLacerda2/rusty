//! `progress` while preparing (#2123): the loader's report, published at a tick head
//! in whole percent of the way to 0.9, as the tap leaves it; then 0.9 once ready.

use super::fixture::world_at;
use super::{GameWorld, Inline, SceneLoader};
use crate::TickTap;
use rusty_core::paths;
use rusty_scene::runtime::prepare::PreparedScene;
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// A loader whose job is `step`% further at each ask, finished at 100%.
struct Steps {
    inner: Inline,
    step: f32,
    asked: f32,
}

impl SceneLoader for Steps {
    fn start(&mut self, op: u32, path: String) {
        self.inner.start(op, path);
    }
    fn finished(&mut self, op: u32) -> bool {
        self.asked += self.step;
        self.asked >= 1.0 && self.inner.finished(op)
    }
    fn take(&mut self, op: u32) -> Result<PreparedScene, String> {
        self.inner.take(op)
    }
    fn cancel(&mut self) {
        self.inner.cancel();
    }
    fn progress(&mut self, _op: u32) -> f32 {
        self.asked
    }
}

/// A tap that hands back `forced` for every move of the progress, as a replay would.
struct Forced(Option<u8>);

impl TickTap for Forced {
    fn at(&mut self, _: crate::TapPoint, _: &mut GameWorld) {}
    fn load_progress(&mut self, _moved: Option<u8>) -> Option<u8> {
        self.0
    }
}

fn playing(tag: &str) -> (GameWorld, String) {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_2123_progress_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = paths::to_slash(&dir.join("level.scene"));
    let mut scene = Scene::new();
    scene.add_entity("Level".into());
    scene.save_to_file(&path).unwrap();
    let (mut gw, _) = world_at(&path);
    gw.set_playing(true);
    gw.tick(DT);
    (gw, path)
}

fn progress_after_ticks(gw: &mut GameWorld, path: &str, ticks: usize) -> Vec<String> {
    let lua = gw.script_manager();
    let held = format!("op = Scene.LoadAsync('{path}'); Scene.SetAllowSceneActivation(op, false)");
    lua.eval(&held).unwrap();
    (0..ticks)
        .map(|_| {
            gw.tick(DT);
            let lua = gw.script_manager();
            lua.eval("return Scene.GetLoadProgress(op)").unwrap()
        })
        .collect()
}

#[test]
fn progress_moves_toward_0_9_by_the_loaders_report_then_holds_there_when_ready() {
    let (mut gw, path) = playing("moves");
    let steps = Steps {
        inner: Inline::default(),
        step: 0.25,
        asked: 0.0,
    };
    gw.set_scene_loader(Box::new(steps));
    let read = progress_after_ticks(&mut gw, &path, 5);
    assert_eq!(
        read,
        ["0.225", "0.45", "0.675", "0.9", "0.9"],
        "held at 0.9"
    );
}

#[test]
fn a_replay_hands_back_the_recorded_progress_not_the_loaders() {
    let (mut gw, path) = playing("forced");
    let steps = Steps {
        inner: Inline::default(),
        step: 0.1,
        asked: 0.0,
    };
    gw.set_scene_loader(Box::new(steps));
    gw.set_tap(Some(Box::new(Forced(Some(50)))));
    assert_eq!(progress_after_ticks(&mut gw, &path, 2), ["0.45", "0.45"]);
    gw.set_tap(Some(Box::new(Forced(None))));
    gw.tick(DT);
    let lua = gw.script_manager();
    assert_eq!(
        lua.eval("return Scene.GetLoadProgress(op)").unwrap(),
        "0.45"
    );
}

#[test]
fn the_prepare_counts_the_steps_it_runs_a_saved_navmesh_skipping_the_bake() {
    use crate::{prepare_counted, PrepareSteps};
    let dir = rusty_test_support::temp::dir().join("rusty_2123_steps");
    std::fs::create_dir_all(&dir).unwrap();
    let path = paths::to_slash(&dir.join("level.scene"));
    Scene::new().save_to_file(&path).unwrap();
    let run = || {
        let steps = PrepareSteps::default();
        prepare_counted(&path, &Default::default(), &steps).unwrap();
        (steps.total(), steps.fraction())
    };
    assert_eq!(run(), (4, 1.0), "no navmesh: it is baked");
    std::fs::write(rusty_scene::nav_settings::navmesh_path(&path), b"saved").unwrap();
    assert_eq!(run(), (3, 1.0), "saved: three steps, all of them done");
}
