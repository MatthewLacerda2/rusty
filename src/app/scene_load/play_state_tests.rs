//! One play state (#1302): editor Play — the transport's `set_playing`, then a
//! paused frame's `poll_transition` — is Play to the Lua API, and what a paused
//! Play queued is dropped on Stop (#1298).

use crate::core::paths;
use crate::scene::Scene;

use super::fixture::world_at;

/// A scene file holding one entity `Keep`, under a per-test temp dir.
fn write_scene(tag: &str) -> String {
    let dir = crate::test_temp::dir().join(format!("rusty_1302_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = paths::to_slash(&dir.join("keep.scene"));
    let mut scene = Scene::new();
    scene.add_entity("Keep".to_string());
    scene.save_to_file(&path).unwrap();
    path
}

#[test]
fn editor_play_is_play_to_the_lua_api() {
    let path = write_scene("api");
    let (mut gw, _) = world_at(&path);
    let keep = gw.scene().borrow().find_entity_by_name("Keep").unwrap();
    gw.set_playing(true);
    gw.poll_transition();
    let eval = |code: String| gw.script_manager().eval(&code).unwrap();
    eval(format!("Scene.Load('{path}')"));
    eval(format!("Scene.DontDestroyOnLoad({keep})"));
    eval(format!("Scene.DestroyEntity({keep})"));
    let scene = gw.scene().borrow();
    assert_eq!(
        scene.pending_load.as_deref(),
        Some(path.as_str()),
        "deferred"
    );
    assert!(scene.persistent.contains(&keep), "DontDestroyOnLoad took");
    assert!(scene.world.contains(keep), "live until the destroy phase");
    assert_eq!(scene.pending_destroy, vec![keep], "queued");
}

#[test]
fn a_destroy_queued_in_a_paused_play_does_not_outlive_stop() {
    let path = write_scene("stop");
    let (mut gw, _) = world_at(&path);
    let keep = gw.scene().borrow().find_entity_by_name("Keep").unwrap();
    gw.set_playing(true);
    gw.poll_transition();
    let destroy = format!("Scene.DestroyEntity({keep})");
    gw.script_manager().eval(&destroy).unwrap();
    gw.set_playing(false);
    gw.poll_transition();
    assert!(gw.scene().borrow().pending_destroy.is_empty());
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    assert!(
        gw.scene().borrow().world.contains(keep),
        "the edit entity survives"
    );
}
