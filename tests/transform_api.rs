//! Integration coverage for the `Transform` scripting namespace (issue #350):
//! position/rotation/scale round-trip through the live scene, a missing entity
//! yields the documented defaults, and `MoveTowards` steps then snaps.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::Scene;

fn scene_with_entity() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Thing".to_string());
    (RefCell::new(scene), id)
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0e-3
}

#[test]
fn position_rotation_scale_round_trip() {
    let lua = Lua::new();
    let (scene, id) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::transform::register(&lua, scope, &scene).unwrap();
        lua.load(format!(
            "Transform.SetPosition({id}, 1, 2, 3)\n\
             Transform.SetRotation({id}, 0, 45, 0)\n\
             Transform.SetScale({id}, 2, 2, 2)"
        ))
        .exec()
        .unwrap();

        let (x, y, z): (f32, f32, f32) = lua
            .load(format!("return Transform.GetPosition({id})"))
            .eval()
            .unwrap();
        assert_eq!((x, y, z), (1.0, 2.0, 3.0));

        let (_, ry, _): (f32, f32, f32) = lua
            .load(format!("return Transform.GetRotation({id})"))
            .eval()
            .unwrap();
        assert!(approx(ry, 45.0), "expected yaw ~45, got {ry}");

        let (sx, sy, sz): (f32, f32, f32) = lua
            .load(format!("return Transform.GetScale({id})"))
            .eval()
            .unwrap();
        assert_eq!((sx, sy, sz), (2.0, 2.0, 2.0));
        Ok(())
    })
    .unwrap();
}

#[test]
fn missing_entity_gets_defaults_and_sets_are_noops() {
    let lua = Lua::new();
    let (scene, _) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::transform::register(&lua, scope, &scene).unwrap();
        let pos: (f32, f32, f32) = lua
            .load("return Transform.GetPosition(999)")
            .eval()
            .unwrap();
        assert_eq!(pos, (0.0, 0.0, 0.0));
        let rot: (f32, f32, f32) = lua
            .load("return Transform.GetRotation(999)")
            .eval()
            .unwrap();
        assert_eq!(rot, (0.0, 0.0, 0.0));
        let scl: (f32, f32, f32) = lua.load("return Transform.GetScale(999)").eval().unwrap();
        assert_eq!(scl, (1.0, 1.0, 1.0), "missing entity reports unit scale");

        // Setters on a missing id must be silent no-ops, never errors.
        lua.load("Transform.SetPosition(999, 5, 5, 5); Transform.SetScale(999, 9, 9, 9)")
            .exec()
            .unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn move_towards_steps_by_step_then_snaps_at_target() {
    let lua = Lua::new();
    let (scene, id) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::transform::register(&lua, scope, &scene).unwrap();

        // 10 units away, stepping 4: two part-steps, then the remainder (2 < 4) snaps.
        lua.load(format!("Transform.MoveTowards({id}, 10, 0, 0, 4)"))
            .exec()
            .unwrap();
        let (x, _, _): (f32, f32, f32) = lua
            .load(format!("return Transform.GetPosition({id})"))
            .eval()
            .unwrap();
        assert!(approx(x, 4.0), "one step of 4, got {x}");

        lua.load(format!(
            "Transform.MoveTowards({id}, 10, 0, 0, 4)\n\
             Transform.MoveTowards({id}, 10, 0, 0, 4)"
        ))
        .exec()
        .unwrap();
        let (x, y, z): (f32, f32, f32) = lua
            .load(format!("return Transform.GetPosition({id})"))
            .eval()
            .unwrap();
        assert_eq!(
            (x, y, z),
            (10.0, 0.0, 0.0),
            "snapped exactly onto the target"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn move_towards_holds_position_when_already_at_target() {
    let lua = Lua::new();
    let (scene, id) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::transform::register(&lua, scope, &scene).unwrap();
        lua.load(format!(
            "Transform.SetPosition({id}, 7, 0, 0)\n\
             Transform.MoveTowards({id}, 7, 0, 0, 0.5)"
        ))
        .exec()
        .unwrap();
        let (x, _, _): (f32, f32, f32) = lua
            .load(format!("return Transform.GetPosition({id})"))
            .eval()
            .unwrap();
        assert_eq!(x, 7.0, "zero-distance move must not drift");
        Ok(())
    })
    .unwrap();
}
