//! `Transform`'s world-space surface (#1196) under a turned, scaled, mirrored parent:
//! every getter and space map agrees with `ecs::World`'s answer, and each setter
//! round-trips its getter in world space.

use std::cell::RefCell;

use glam::{EulerRot, Quat, Vec3};
use mlua::Lua;
use rusty::scene::Scene;

/// A child at local `(1, 2, 3)` under a parent turned 90° about Y, scaled
/// `(2, 3, -1)` (a mirror) and moved to `(5, 0, 0)`.
pub(super) fn hierarchy() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let parent = scene.add_entity("Parent".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(parent)).unwrap();
    {
        let mut t = scene.world.transform_mut(parent).unwrap();
        t.position = Vec3::new(5.0, 0.0, 0.0);
        t.rotation = Quat::from_rotation_y(90f32.to_radians());
        t.scale = Vec3::new(2.0, 3.0, -1.0);
    }
    {
        let mut t = scene.world.transform_mut(child).unwrap();
        t.position = Vec3::new(1.0, 2.0, 3.0);
        t.rotation = Quat::from_rotation_x(0.3);
    }
    (RefCell::new(scene), parent, child)
}

/// Run `body` with `Transform` bound over `scene`; `get(lua)` evaluates a vector.
fn with_lua(scene: &RefCell<Scene>, body: impl FnOnce(&dyn Fn(&str) -> Vec3)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::transform::register(&lua, scope, scene).unwrap();
        let get = |code: &str| -> Vec3 {
            let (x, y, z): (f32, f32, f32) = lua.load(code).eval().unwrap();
            Vec3::new(x, y, z)
        };
        body(&get);
        Ok(())
    })
    .unwrap();
}

fn near(a: Vec3, b: Vec3) -> bool {
    a.abs_diff_eq(b, 1e-4)
}

#[test]
fn getters_and_maps_read_world_space_through_the_parent() {
    let (scene, _, c) = hierarchy();
    let (pos, scale) = {
        let s = scene.borrow();
        (s.world.world_position(c), s.world.lossy_scale(c))
    };
    let (point, dir) = {
        let s = scene.borrow();
        let p = Vec3::new(0.5, -1.0, 2.0);
        (
            s.world.transform_point(c, p),
            s.world.transform_direction(c, p),
        )
    };
    with_lua(&scene, |get| {
        assert!(near(
            get(&format!("return Transform.GetPosition({c})")),
            pos
        ));
        assert_eq!(
            get(&format!("return Transform.GetLocalPosition({c})")),
            Vec3::new(1.0, 2.0, 3.0)
        );
        assert!(near(
            get(&format!("return Transform.GetLossyScale({c})")),
            scale
        ));
        assert!(
            scale.x < 0.0 || scale.y < 0.0 || scale.z < 0.0,
            "the mirror shows: {scale}"
        );
        assert_eq!(
            get(&format!("return Transform.GetLocalScale({c})")),
            Vec3::ONE
        );
        assert!(near(
            get(&format!("return Transform.TransformPoint({c}, 0.5, -1, 2)")),
            point
        ));
        assert!(near(
            get(&format!(
                "return Transform.TransformDirection({c}, 0.5, -1, 2)"
            )),
            dir
        ));
        let back = get(&format!(
            "return Transform.InverseTransformPoint({c}, Transform.TransformPoint({c}, 0.5, -1, 2))"
        ));
        assert!(near(back, Vec3::new(0.5, -1.0, 2.0)), "{back}");
    });
}

#[test]
fn setters_round_trip_their_getters_in_world_space() {
    let (scene, _, c) = hierarchy();
    with_lua(&scene, |get| {
        let at = get(&format!(
            "Transform.SetPosition({c}, -4, 7, 0.5); return Transform.GetPosition({c})"
        ));
        assert!(near(at, Vec3::new(-4.0, 7.0, 0.5)), "{at}");
        let stepped = get(&format!(
            "Transform.MoveTowards({c}, -4, 9, 0.5, 1); return Transform.GetPosition({c})"
        ));
        assert!(near(stepped, Vec3::new(-4.0, 8.0, 0.5)), "{stepped}");
        get(&format!(
            "Transform.SetRotation({c}, 10, 30, -20); return 0, 0, 0"
        ));
    });
    let want = Quat::from_euler(
        EulerRot::YXZ,
        30f32.to_radians(),
        10f32.to_radians(),
        -20f32.to_radians(),
    );
    let got = scene.borrow().world.world_rotation(c);
    assert!(
        got.dot(want).abs() > 1.0 - 1e-5,
        "world rotation {got} != {want}"
    );
    assert!(
        near(
            scene.borrow().world.world_position(c),
            Vec3::new(-4.0, 8.0, 0.5)
        ),
        "a turn leaves it where it stands"
    );
}
