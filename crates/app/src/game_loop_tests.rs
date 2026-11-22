use super::game_tests::{world_with, DT};
use rusty_components::Playback;
use rusty_scene::{Scene, ScriptComponent};
use std::cell::RefCell;
use std::rc::Rc;

fn temp_path(name: &str) -> String {
    rusty_core::paths::to_slash(&rusty_test_support::temp::dir().join(name))
}

#[test]
fn runtime_spawned_prefab_scripts_run_from_the_next_tick() {
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
        rusty_edit::save_prefab(&authoring, id, prefab).unwrap();
    }

    let spawner_lua = &temp_path("rusty_322_spawner.lua");
    std::fs::write(
        spawner_lua,
        format!(
            "return {{ Update = function(id, dt)\n\
             if not _G.__spawned then _G.__spawned = true; \
             Scene.Instantiate('{prefab}') end\nend }}"
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

    gw.tick(DT);
    let eval = |gw: &super::GameWorld, line: &str| gw.script_manager().eval(line).unwrap();
    assert_eq!(eval(&gw, "__spawned"), "true");
    assert_eq!(eval(&gw, "__enemy_log"), "nil", "no load in the spawn tick");

    gw.tick(DT);
    assert_eq!(eval(&gw, "__enemy_log"), "ASU");

    gw.tick(DT);
    assert_eq!(eval(&gw, "__enemy_log"), "ASUU");
}

fn rig_scene(anim: rusty_components::AnimatorComponent) -> (Rc<RefCell<Scene>>, u32) {
    let mut s = Scene::new();
    let id = s.add_entity("Rig".to_string());
    s.world.set_animator(id, Some(anim));
    (Rc::new(RefCell::new(s)), id)
}

fn animator_of(scene: &Rc<RefCell<Scene>>, id: u32) -> rusty_components::AnimatorComponent {
    scene.borrow().world.animator(id).unwrap().clone()
}

#[test]
fn play_tick_advances_the_animator_through_the_loop() {
    let (scene, id) = rig_scene(rusty_components::AnimatorComponent {
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

#[test]
fn graph_driven_animator_transitions_through_the_play_loop() {
    use rusty_asset::animation_graph::{self, AnimationGraph};
    let path = rusty_test_support::temp::dir().join("rusty_316_loop.animgraph");
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

    let (scene, id) = rig_scene(rusty_components::AnimatorComponent {
        graph: Some(path.to_string_lossy().into_owned()),
        ..Default::default()
    });
    let mut gw = world_with(Rc::clone(&scene));
    gw.set_playing(true);
    gw.tick(DT);
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
    gw.tick(DT);
    let anim = animator_of(&scene, id);
    assert_eq!(anim.base.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.base.previous_clip.as_deref(), Some("IdleClip"));
    assert_eq!(anim.base.crossfade_duration, 0.25);
}
