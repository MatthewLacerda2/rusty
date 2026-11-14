//! The load timer (#2123): what a swapped-in load reports, and that a load in a Play
//! session's first tick survives the stats reset that tick ends with.

use super::*;
use rusty_core::input::InputState;
use rusty_core::paths;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

#[test]
fn a_load_timer_reports_a_swapped_in_load_and_drops_a_failed_one() {
    let loads = LoadSamples::default();
    let mut timer = LoadTimer::new(Rc::clone(&loads));
    let mut console = ConsoleLogs::new();
    let mut ran = 0;
    timer.step("read", &mut || ran += 1);
    timer.finish(None, &mut console);
    assert!(loads.borrow().is_empty() && console.messages.is_empty());
    timer.step("read", &mut || ran += 1);
    timer.step("physics", &mut || ran += 1);
    timer.finish(Some("a.scene"), &mut console);
    assert_eq!(ran, 3, "each step ran once");
    let keys: Vec<String> = loads.borrow().iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, ["load_read_ms", "load_physics_ms", "load_ms"]);
    let line = &console.messages[0].0;
    assert!(
        line.starts_with("Scene load a.scene: ") && line.contains("(read "),
        "{line}"
    );
}

/// A world on an empty scene with the frame stats installed, and a saved level.
fn world_with_level(tag: &str) -> (GameWorld, Rc<RefCell<FrameStats>>, String) {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_2123_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let level = paths::to_slash(&dir.join("level.scene"));
    let mut lvl = Scene::new();
    lvl.add_entity("Level".to_string());
    lvl.save_to_file(&level).unwrap();
    let mut gw = GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    let stats = install(&mut gw);
    (gw, stats, level)
}

#[test]
fn a_load_on_the_first_play_tick_survives_the_stats_reset() {
    let (mut gw, stats, level) = world_with_level("first_tick");
    gw.set_playing(true);
    gw.scene().borrow_mut().request_load(level);
    gw.tick(1.0 / 60.0); // the load swaps in, then the tick's end resets the stats
    assert!(gw.scene().borrow().find_entity_by_name("Level").is_some());
    let stats = stats.borrow();
    assert_eq!(stats.frames, 1, "the reset ran this tick");
    for key in ["load_ms", "load_read_ms", "load_navmesh_ms"] {
        assert_eq!(stats.get(key).map(|s| s.samples), Some(1), "{key}");
    }
}

#[test]
fn a_later_load_adds_one_sample_and_a_failed_one_none() {
    let (mut gw, stats, level) = world_with_level("later");
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    assert!(
        stats.borrow().get("load_ms").is_none(),
        "absent until a load"
    );
    gw.scene().borrow_mut().request_load(level);
    gw.tick(1.0 / 60.0);
    let missing = "rusty_2123_no_such_dir/gone.scene".to_string();
    gw.scene().borrow_mut().request_load(missing);
    gw.tick(1.0 / 60.0);
    let samples = stats.borrow().get("load_ms").map(|s| s.samples);
    assert_eq!(samples, Some(1));
}
