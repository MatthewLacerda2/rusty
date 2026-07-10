//! Unit tests for the play/stop boundary in `GameWorld` (game.rs): pitch clamping,
//! the transition state machine, the scaled-dt threading, and the
//! snapshot-capture-on-Play / restore-on-Stop invariant. Determinism contract:
//! fixed dt, no wall-clock, no RNG.

use super::*;
use crate::scene::{Scene, ScriptComponent};
use glam::Vec3;
use std::cell::RefCell;
use std::rc::Rc;

fn world_with(scene: Rc<RefCell<Scene>>) -> GameWorld {
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, nav, console)
}

fn empty_world() -> GameWorld {
    world_with(Rc::new(RefCell::new(Scene::new())))
}

const DT: f32 = 1.0 / 60.0;

/// Platform-safe temp path (the Windows runner has no `/tmp`).
fn temp_path(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn editor_fly_pitch_clamps_at_eighty() {
    let mut gw = empty_world();
    gw.camera().borrow_mut().pitch = 79.5;
    gw.input().borrow_mut().set_key_state("UP", true);
    for _ in 0..120 {
        gw.tick(DT);
    }
    assert!((gw.camera().borrow().pitch - 80.0).abs() < 1e-4);

    let mut gw = empty_world();
    gw.camera().borrow_mut().pitch = -79.5;
    gw.input().borrow_mut().set_key_state("DOWN", true);
    for _ in 0..120 {
        gw.tick(DT);
    }
    assert!((gw.camera().borrow().pitch + 80.0).abs() < 1e-4);
}

#[test]
fn transition_is_none_while_steady() {
    let mut gw = empty_world();
    assert!(gw.tick(DT) == PlayTransition::None); // stays in edit mode
    gw.set_playing(true);
    assert!(gw.tick(DT) == PlayTransition::Entered);
    assert!(gw.tick(DT) == PlayTransition::None); // already playing
}

#[test]
fn entering_play_resets_frame_and_time() {
    let mut gw = empty_world();
    // Dirty the clock so we can see the reset on enter_play.
    gw.time().borrow_mut().advance(0.5);
    gw.resources.play_frame = 99;
    gw.set_playing(true);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Entered);
    // enter_play resets frame_count to 0 then the tick's own advance makes it 1.
    assert_eq!(gw.time().borrow().frame_count, 1);
    // advance_frame ran once this tick.
    assert_eq!(gw.play_frame(), 1);
}

#[test]
fn exiting_play_clears_runtime_state() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT); // Entered: builds physics, sets play_frame
    assert!(gw.resources.physics.borrow().is_some());
    gw.resources.pathfinding_points = vec![Vec3::ZERO];
    gw.set_playing(false);
    let t = gw.tick(DT);
    assert!(t == PlayTransition::Exited);
    // exit_play tore physics down, cleared the debug path, reset the counter.
    assert!(gw.resources.physics.borrow().is_none());
    assert!(gw.pathfinding_points().is_empty());
    assert_eq!(gw.play_frame(), 0);
}

#[test]
fn stop_restores_the_edit_scene_snapshot() {
    let mut s = Scene::new();
    let id = s.add_entity("Mover".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 0.0);
    let scene = Rc::new(RefCell::new(s));
    let mut gw = world_with(Rc::clone(&scene));

    gw.set_playing(true);
    gw.tick(DT); // Entered: snapshot captured
                 // Mutate the entity mid-play.
    scene.borrow_mut().world.transform_mut(id).unwrap().position = Vec3::new(9.0, 9.0, 9.0);
    gw.set_playing(false);
    gw.tick(DT); // Exited: restore snapshot
    let restored = scene.borrow().world.transform(id).unwrap().position;
    assert!(
        restored.abs_diff_eq(Vec3::new(1.0, 0.0, 0.0), 1e-6),
        "play-mode mutation leaked into the edit scene: {restored:?}"
    );
}

#[test]
fn tick_scales_dt_by_time_scale() {
    let mut gw = empty_world();
    gw.set_playing(true);
    gw.tick(DT); // Entered resets clock; advance applies scale 1.0
    gw.time().borrow_mut().set_time_scale(0.5);
    gw.tick(DT);
    // The schedule sees the scaled delta the tick stored in frame_dt.
    assert!((gw.resources.dt() - DT * 0.5).abs() < 1e-7);
    // Raw (unscaled) delta is preserved on the clock.
    assert!((gw.time().borrow().unscaled_delta_time - DT).abs() < 1e-7);
}

#[test]
fn play_frame_accumulates_each_tick() {
    let mut gw = empty_world();
    gw.set_playing(true);
    for _ in 0..5 {
        gw.tick(DT);
    }
    // enter_play set 0, then five ticks each run advance_frame once.
    assert_eq!(gw.play_frame(), 5);
}

/// End-to-end #322: a prefab spawned from inside a script's `Update` gets its
/// own script loaded at the head of the NEXT tick's script phase, where Awake,
/// Start and its first Update all run — the spawn tick itself never touches it.
#[test]
fn runtime_spawned_prefab_scripts_run_from_the_next_tick() {
    // Author the prefab: one entity carrying a callback-logging script.
    let enemy_lua = &temp_path("rusty_322_enemy.lua");
    std::fs::write(
        enemy_lua,
        "_G.__enemy_log = _G.__enemy_log or ''\nreturn {\n\
         Awake = function(id) __enemy_log = __enemy_log .. 'A' end,\n\
         Start = function(id) __enemy_log = __enemy_log .. 'S' end,\n\
         Update = function(id, dt) __enemy_log = __enemy_log .. 'U' end,\n}",
    )
    .unwrap();
    let prefab = &temp_path("rusty_322_enemy.prefab");
    {
        let mut authoring = Scene::new();
        let id = authoring.add_entity("Enemy".to_string());
        *authoring.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
            path: enemy_lua.to_string(),
            ..Default::default()
        }];
        crate::scene::save_prefab(&authoring, id, prefab).unwrap();
    }

    // The live scene: a spawner whose first Update instantiates that prefab.
    let spawner_lua = &temp_path("rusty_322_spawner.lua");
    // Forward slashes in the Lua literal: a Windows `\` would be a Lua escape.
    let prefab_lua = prefab.replace('\\', "/");
    std::fs::write(
        spawner_lua,
        format!(
            "return {{ Update = function(id, dt)\n\
             if not _G.__spawned then _G.__spawned = true; \
             Scene.Instantiate('{prefab_lua}') end\nend }}"
        ),
    )
    .unwrap();
    let mut s = Scene::new();
    let id = s.add_entity("Spawner".to_string());
    *s.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: spawner_lua.to_string(),
        ..Default::default()
    }];
    let mut gw = world_with(Rc::new(RefCell::new(s)));
    gw.set_playing(true);

    gw.tick(DT); // spawn tick: the enemy entity exists, its script untouched
    let eval = |gw: &GameWorld, line: &str| gw.script_manager().eval(line).unwrap();
    assert_eq!(eval(&gw, "__spawned"), "true");
    assert_eq!(eval(&gw, "__enemy_log"), "nil", "no load in the spawn tick");

    gw.tick(DT); // next tick: queued load drains → Awake, Start, first Update
    assert_eq!(eval(&gw, "__enemy_log"), "ASU");

    gw.tick(DT); // steady state: Update only, init never re-fires
    assert_eq!(eval(&gw, "__enemy_log"), "ASUU");
}

/// A one-entity scene carrying `anim`; returns (scene handle, entity id).
fn rig_scene(anim: crate::components::AnimatorComponent) -> (Rc<RefCell<Scene>>, u32) {
    let mut s = Scene::new();
    let id = s.add_entity("Rig".to_string());
    s.world.set_animator(id, Some(anim));
    (Rc::new(RefCell::new(s)), id)
}

fn animator_of(scene: &Rc<RefCell<Scene>>, id: u32) -> crate::components::AnimatorComponent {
    scene.borrow().world.animator(id).unwrap().clone()
}

/// #312 mutation audit: the `animate` system must actually run inside the play
/// loop's `FixedUpdate` dispatch — stepped ticks visibly advance animator state.
#[test]
fn play_tick_advances_the_animator_through_the_loop() {
    let (scene, id) = rig_scene(crate::components::AnimatorComponent {
        current_clip: "Walk".to_string(),
        is_playing: true,
        ..Default::default()
    });
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
    gw.tick(DT);
    let time = animator_of(&scene, id).time;
    assert_eq!(time, 2.0 * DT, "two stepped frames advanced the playhead");
}

/// End-to-end #316: a graph-referencing animator binds (entry node) and then
/// transitions from a parameter change, all through `GameWorld::tick`'s dispatch.
#[test]
fn graph_driven_animator_transitions_through_the_play_loop() {
    use crate::asset::animation_graph::{self, AnimationGraph};
    let path = std::env::temp_dir().join("rusty_316_loop.animgraph");
    let graph: AnimationGraph = serde_json::from_value(serde_json::json!({
        "parameters": { "speed": { "Float": 0.0 } },
        "nodes": [
            { "name": "Idle", "clip": "IdleClip", "is_loop": true },
            { "name": "Run", "clip": "RunClip", "is_loop": true }
        ],
        "edges": [ {
            "from": "Idle", "to": "Run", "transition_duration": 0.25,
            "conditions": [ { "Float": { "parameter": "speed", "op": "Greater", "value": 1.0 } } ]
        } ],
        "entry": "Idle"
    }))
    .unwrap();
    animation_graph::save(&path, &graph).unwrap();

    let (scene, id) = rig_scene(crate::components::AnimatorComponent {
        graph: Some(path.to_string_lossy().into_owned()),
        ..Default::default()
    });
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT); // first step binds: seeds "speed" = 0.0 and enters Idle
    let anim = animator_of(&scene, id);
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
    assert_eq!(anim.current_clip, "IdleClip");
    assert_eq!(anim.get_float("speed"), Some(0.0), "default seeded");
    scene
        .borrow_mut()
        .world
        .animator_mut(id)
        .unwrap()
        .set_float("speed", 2.0);
    gw.tick(DT); // the Idle → Run edge fires and crossfades
    let anim = animator_of(&scene, id);
    assert_eq!(anim.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.previous_clip.as_deref(), Some("IdleClip"));
    assert_eq!(anim.crossfade_duration, 0.25);
}

#[test]
fn snap_camera_to_player_positions_behind_player() {
    let mut s = Scene::new();
    let id = s.add_entity("Player".to_string());
    s.world.transform_mut(id).unwrap().position = Vec3::new(2.0, 0.0, 3.0);
    let mut gw = world_with(Rc::new(RefCell::new(s)));
    gw.set_playing(true);
    gw.tick(DT); // enter_play calls snap_camera_to_player
    let cam = gw.camera().borrow();
    assert!(cam.position.abs_diff_eq(Vec3::new(2.0, 1.5, -1.5), 1e-5));
    assert!((cam.yaw - 90.0).abs() < 1e-5);
    assert!((cam.pitch + 10.0).abs() < 1e-5);
}
