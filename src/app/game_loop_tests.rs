//! What `GameWorld::tick`'s play loop runs (game.rs): a runtime-spawned prefab's
//! scripts and the animators, each driven end to end through the real dispatch.

use super::game_tests::{world_with, DT};
use crate::components::Playback;
use crate::scene::{Scene, ScriptComponent};
use std::cell::RefCell;
use std::rc::Rc;

/// Platform-safe temp path (the Windows runner has no `/tmp`).
fn temp_path(name: &str) -> String {
    format!("{}/{name}", crate::test_temp::dir().display())
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
    let eval = |gw: &super::GameWorld, line: &str| gw.script_manager().eval(line).unwrap();
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
        base: Playback {
            current_clip: "Walk".to_string(),
            ..Default::default()
        },
        is_playing: true,
        ..Default::default()
    });
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
    gw.tick(DT);
    let time = animator_of(&scene, id).base.time;
    assert_eq!(time, 2.0 * DT, "two stepped frames advanced the playhead");
}

/// End-to-end #316: a graph-referencing animator binds (entry node) and then
/// transitions from a parameter change, all through `GameWorld::tick`'s dispatch.
#[test]
fn graph_driven_animator_transitions_through_the_play_loop() {
    use crate::asset::animation_graph::{self, AnimationGraph};
    let path = crate::test_temp::dir().join("rusty_316_loop.animgraph");
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
    assert_eq!(anim.base.current_node.as_deref(), Some("Idle"));
    assert_eq!(anim.base.current_clip, "IdleClip");
    assert_eq!(anim.get_float("speed"), Some(0.0), "default seeded");
    scene
        .borrow_mut()
        .world
        .animator_mut(id)
        .unwrap()
        .set_float("speed", 2.0);
    gw.tick(DT); // the Idle → Run edge fires and crossfades
    let anim = animator_of(&scene, id);
    assert_eq!(anim.base.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.base.previous_clip.as_deref(), Some("IdleClip"));
    assert_eq!(anim.base.crossfade_duration, 0.25);
}
