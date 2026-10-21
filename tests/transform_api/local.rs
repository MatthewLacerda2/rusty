//! `Transform`'s rotations and local setters under the turned, scaled, mirrored
//! parent of `world.rs` (#1196): world and local differ there, so a getter or
//! setter bound to the other space's function fails here.

use glam::{EulerRot, Quat, Vec3};

use super::world::{hierarchy, near, with_lua};

/// The rotation a Lua Euler triple (degrees, Unity's YXZ order) names.
fn quat(v: Vec3) -> Quat {
    let r = v * std::f32::consts::PI / 180.0;
    Quat::from_euler(EulerRot::YXZ, r.y, r.x, r.z)
}

fn same(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-4
}

#[test]
fn rotations_read_their_own_space() {
    let (scene, _, c) = hierarchy();
    let world = scene.borrow().world.world_rotation(c);
    let local = Quat::from_rotation_x(0.3);
    assert!(!same(world, local), "the parent turns the child");
    with_lua(&scene, |get| {
        let w = quat(get(&format!("return Transform.GetRotation({c})")));
        assert!(same(w, world), "{w} != {world}");
        let l = quat(get(&format!("return Transform.GetLocalRotation({c})")));
        assert!(same(l, local), "{l} != {local}");
    });
}

#[test]
fn set_rotation_round_trips_in_world_space() {
    let (scene, _, c) = hierarchy();
    with_lua(&scene, |get| {
        let r = get(&format!(
            "Transform.SetRotation({c}, 10, 30, -20); return Transform.GetRotation({c})"
        ));
        let want = quat(Vec3::new(10.0, 30.0, -20.0));
        assert!(same(quat(r), want), "{r}");
    });
    let local = scene.borrow().world.transform(c).unwrap().rotation;
    assert!(
        !same(local, quat(Vec3::new(10.0, 30.0, -20.0))),
        "stored local"
    );
}

#[test]
fn local_setters_write_local_values() {
    let (scene, _, c) = hierarchy();
    with_lua(&scene, |get| {
        get(&format!(
            "Transform.SetLocalPosition({c}, 4, 5, 6)\n\
             Transform.SetLocalRotation({c}, 10, 20, 30)\n\
             Transform.SetLocalScale({c}, 2, 2, 2)\n\
             return 0, 0, 0"
        ));
        let at = get(&format!("return Transform.GetLocalPosition({c})"));
        assert!(near(at, Vec3::new(4.0, 5.0, 6.0)), "{at}");
        let world = get(&format!("return Transform.GetPosition({c})"));
        assert!(!near(world, at), "the parent moves it: {world}");
    });
    let s = scene.borrow();
    let t = s.world.transform(c).unwrap();
    assert!(near(t.position, Vec3::new(4.0, 5.0, 6.0)), "{}", t.position);
    let want = quat(Vec3::new(10.0, 20.0, 30.0));
    assert!(same(t.rotation, want), "{}", t.rotation);
    assert!(!same(s.world.world_rotation(c), want), "world is turned");
    assert_eq!(t.scale, Vec3::splat(2.0));
    assert!(
        !near(s.world.lossy_scale(c), Vec3::splat(2.0)),
        "lossy differs"
    );
}
