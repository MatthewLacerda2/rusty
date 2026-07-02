//! Animator parameters (#314) end-to-end: the `Animator.Set*` Lua setters write the
//! component's typed store, a wrong-typed set is a warned no-op (never an error),
//! and every parameter kind round-trips through `SceneData` (the same
//! `to_scene_data` → `apply_scene_data` document a scene save/load uses).

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{AnimatorComponent, AnimatorParameter};
use rusty::scene::{apply_scene_data, to_scene_data, Scene};
use rusty::scripting::ConsoleLogs;

/// A scene holding one entity with a default animator; returns its id.
fn scene_with_animator() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Rig".to_string());
    scene
        .get_entity_mut(id)
        .expect("entity just added")
        .animator = Some(AnimatorComponent::default());
    (RefCell::new(scene), id)
}

/// Register the `Animator` namespace against `scene` and run `script`.
fn run(scene: &RefCell<Scene>, console: &RefCell<ConsoleLogs>, script: &str) {
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, console).unwrap();
        lua.load(script).exec().unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn lua_setters_write_every_parameter_kind() {
    let (scene, id) = scene_with_animator();
    let console = RefCell::new(ConsoleLogs::new());
    run(
        &scene,
        &console,
        &format!(
            r#"Animator.SetBool({id}, "isGrounded", true)
               Animator.SetFloat({id}, "speed", 4.2)
               Animator.SetInt({id}, "weapon", 2)
               Animator.SetTrigger({id}, "Jump")"#
        ),
    );
    let scene = scene.borrow();
    let anim = scene.get_entity(id).unwrap().animator.clone().unwrap();
    assert_eq!(anim.get_bool("isGrounded"), Some(true));
    assert_eq!(anim.get_float("speed"), Some(4.2));
    assert_eq!(anim.get_int("weapon"), Some(2));
    assert!(
        anim.trigger_is_set("Jump"),
        "SetTrigger latches the one-shot"
    );
}

#[test]
fn wrong_typed_lua_set_is_a_warned_no_op() {
    let (scene, id) = scene_with_animator();
    let console = RefCell::new(ConsoleLogs::new());
    run(
        &scene,
        &console,
        &format!(
            r#"Animator.SetBool({id}, "isGrounded", true)
               Animator.SetFloat({id}, "isGrounded", 1.0)"#
        ),
    );
    let scene = scene.borrow();
    let anim = scene.get_entity(id).unwrap().animator.clone().unwrap();
    assert_eq!(
        anim.get_bool("isGrounded"),
        Some(true),
        "the Bool survives the refused Float write"
    );
    let warned = console
        .borrow()
        .messages
        .iter()
        .any(|(msg, _)| msg.contains("isGrounded"));
    assert!(warned, "the refused set is surfaced in the console");
}

#[test]
fn setter_on_missing_entity_or_animator_is_a_silent_no_op() {
    let (scene, _) = scene_with_animator();
    let console = RefCell::new(ConsoleLogs::new());
    let id = scene.borrow_mut().add_entity("NoAnimator".to_string());
    // Neither the animator-less entity nor a bogus id may error.
    run(
        &scene,
        &console,
        &format!(
            r#"Animator.SetFloat({id}, "speed", 1.0)
               Animator.SetTrigger(9999, "Jump")"#
        ),
    );
    assert!(scene.borrow().get_entity(id).unwrap().animator.is_none());
}

#[test]
fn every_parameter_kind_round_trips_through_scene_data() {
    let (scene, id) = scene_with_animator();
    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.get_entity_mut(id).unwrap();
        let anim = e.animator.as_mut().unwrap();
        anim.set_bool("isGrounded", true);
        anim.set_float("speed", 4.2);
        anim.set_int("weapon", 2);
        anim.set_trigger("Jump");
    }

    // Through the on-disk JSON itself, so the round-trip covers serde both ways.
    let json = serde_json::to_string(&to_scene_data(&scene.borrow())).unwrap();
    let data = serde_json::from_str(&json).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);

    let anim = loaded.get_entity(id).unwrap().animator.clone().unwrap();
    assert_eq!(
        anim.parameters.into_iter().collect::<Vec<_>>(),
        [
            ("Jump".to_string(), AnimatorParameter::Trigger(true)),
            ("isGrounded".to_string(), AnimatorParameter::Bool(true)),
            ("speed".to_string(), AnimatorParameter::Float(4.2)),
            ("weapon".to_string(), AnimatorParameter::Int(2)),
        ],
        "all four kinds survive save/load, name-sorted (deterministic order)"
    );
}
