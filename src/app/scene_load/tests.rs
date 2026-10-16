//! `Scene.Load` through the real `GameWorld` play loop (#432): the outgoing scene's
//! unload lifecycle, `DontDestroyOnLoad` survivors keeping their ids and timers, the
//! incoming scene initialising next tick, and Stop restoring the scene Play began in.
//! Callbacks `print` to the shared console, which is where each test reads them.

use super::fixture::{logs, world_at, write_scenes};
use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

fn at(log: &[String], prefix: &str) -> Option<usize> {
    log.iter().position(|m| m.starts_with(prefix))
}

fn id_of(gw: &GameWorld, name: &str) -> Option<u32> {
    gw.scene().borrow().find_entity_by_name(name)
}

#[test]
fn load_unloads_the_old_scene_and_carries_survivors() {
    let (menu, level) = write_scenes("unload");
    let (mut gw, console) = world_at(&menu);
    gw.set_playing(true);
    let music = id_of(&gw, "Music").unwrap();

    gw.tick(DT); // Menu's first Update requests the load; the tail swaps.
    let log = logs(&console);
    let (disable, destroy) = (at(&log, "menu:disable"), at(&log, "menu:destroy true"));
    assert!(
        disable.is_some() && disable < destroy,
        "OnDisable then OnDestroy: {log:?}"
    );
    assert!(
        at(&log, "music:destroy").is_none(),
        "a survivor is not torn down"
    );
    assert!(
        at(&log, "level:awake").is_none(),
        "the new scene inits next tick"
    );
    assert_eq!(id_of(&gw, "Menu"), None);
    assert_eq!(
        id_of(&gw, "Music"),
        Some(music),
        "the survivor keeps its id"
    );
    assert!(id_of(&gw, "Level").is_some());
    assert_eq!(
        gw.script_manager()
            .eval("return Scene.GetActivePath()")
            .unwrap(),
        level
    );

    for _ in 0..10 {
        gw.tick(DT);
    }
    let log = logs(&console);
    let (awake, start) = (at(&log, "level:awake"), at(&log, "level:start"));
    assert!(
        awake.is_some() && awake < start,
        "Awake then Start: {log:?}"
    );
    assert!(
        at(&log, "menu:timer").is_none(),
        "an unloaded owner's timer dies with it"
    );
    assert!(
        at(&log, "music:tick").is_some(),
        "a survivor's timer keeps running"
    );
}

#[test]
fn stop_restores_the_scene_play_started_in() {
    let (menu, _) = write_scenes("stop");
    let (mut gw, _) = world_at(&menu);
    let id = gw.scene().borrow().id();
    gw.set_playing(true);
    gw.tick(DT);
    gw.tick(DT);
    assert!(id_of(&gw, "Level").is_some());

    gw.set_playing(false);
    gw.poll_transition();
    assert!(id_of(&gw, "Menu").is_some() && id_of(&gw, "Level").is_none());
    let path = gw.script_manager().scene_path_cell().borrow().clone();
    assert_eq!(path.as_deref(), Some(menu.as_str()), "and its scene file");
    let scene = gw.scene().borrow();
    assert!(scene.persistent.is_empty() && scene.pending_load.is_none());
    assert_ne!(
        scene.id(),
        id,
        "the renderer must not reuse the played scene's caches"
    );
}
