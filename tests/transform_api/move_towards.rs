//! `Transform.MoveTowards` steps, snaps and backs away (#350, #997).

use mlua::Lua;

use super::{approx, scene_with_entity};

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
        let x = || -> f32 {
            lua.load(format!("return Transform.GetPosition({id})"))
                .eval()
                .unwrap()
        };
        assert_eq!(x(), 7.0, "zero-distance move must not drift");
        // A negative step backs away, like Unity's `Vector3.MoveTowards` (#997).
        lua.load(format!("Transform.MoveTowards({id}, 10, 0, 0, -1)"))
            .exec()
            .unwrap();
        assert_eq!(x(), 6.0);
        Ok(())
    })
    .unwrap();
}
