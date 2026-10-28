//! `Animator.SetSpeed` / `GetSpeed` (#1342) end-to-end through Lua: the write
//! lands on the component's `speed` multiplier (the field the sampler reads),
//! any finite value is accepted as the inspector's field accepts it, NaN and
//! infinity raise a Lua error, and a missing animator reads `nil`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

use super::{run, scene_with_animator};

#[test]
fn set_speed_writes_the_component_multiplier() {
    let (scene, id) = scene_with_animator();
    assert_eq!(run(&scene, &format!("Animator.GetSpeed({id})")), "1.0");
    for (speed, expect) in [("2.5", 2.5), ("0", 0.0), ("-1", -1.0)] {
        run(&scene, &format!("Animator.SetSpeed({id}, {speed})"));
        assert_eq!(scene.borrow().world.animator(id).unwrap().speed, expect);
        assert_eq!(
            run(&scene, &format!("Animator.GetSpeed({id})")),
            format!("{expect:?}")
        );
    }
}

#[test]
fn add_component_starts_at_authored_speed() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("Rig".to_string());
    let path = RefCell::new(None);
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::scene::register(&lua, s, &scene, &path).unwrap();
        lua.load(format!("Scene.AddComponent({id}, 'Animator')"))
            .exec()
    })
    .unwrap();
    assert_eq!(run(&scene, &format!("Animator.GetSpeed({id})")), "1.0");
}

#[test]
fn speed_survives_play_and_crossfade() {
    let (scene, id) = scene_with_animator();
    run(&scene, &format!("Animator.SetSpeed({id}, 3)"));
    run(&scene, &format!("Animator.Play({id}, 'Fire')"));
    run(&scene, &format!("Animator.Crossfade({id}, 'Idle', 0.2)"));
    assert_eq!(scene.borrow().world.animator(id).unwrap().speed, 3.0);
}

#[test]
fn non_finite_speeds_raise_and_leave_the_speed_alone() {
    let (scene, id) = scene_with_animator();
    for bad in ["0/0", "math.huge", "-math.huge"] {
        let err = eval(&scene, &format!("Animator.SetSpeed({id}, {bad})")).unwrap_err();
        assert!(err.contains("finite"), "{bad}: {err}");
    }
    assert_eq!(scene.borrow().world.animator(id).unwrap().speed, 1.0);
}

#[test]
fn a_missing_animator_reads_nil_and_ignores_writes() {
    let scene = RefCell::new(Scene::new());
    let id = scene.borrow_mut().add_entity("Bare".to_string());
    assert_eq!(run(&scene, &format!("Animator.SetSpeed({id}, 2)")), "nil");
    assert_eq!(run(&scene, &format!("Animator.GetSpeed({id})")), "nil");
    assert_eq!(run(&scene, "Animator.GetSpeed(9999)"), "nil");
}

/// Run `script` against `scene`, returning the Lua error's message on failure.
fn eval(scene: &RefCell<Scene>, script: &str) -> Result<(), String> {
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, &console).unwrap();
        lua.load(script).exec()
    })
    .map_err(|e| e.to_string())
}
