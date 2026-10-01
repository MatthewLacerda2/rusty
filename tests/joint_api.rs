//! Integration coverage for the `Joint` namespace (#449): every setter writes
//! through the shared ops (clamped, unknown names ignored), the getters read it
//! back, the component survives a scene save → load, and its connected body
//! follows the entity through a prefab extract → stamp.

use std::cell::RefCell;

use glam::{Vec2, Vec3};
use mlua::Lua;
use rusty::components::{Entity, JointComponent, JointKind};
use rusty::scene::prefab::extract_prefab;
use rusty::scene::{instantiate_prefab, Scene};

fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::joint::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

fn eval(lua: &Lua, code: &str) -> String {
    let v: mlua::MultiValue = lua.load(code).eval().unwrap();
    let parts: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
    parts.join(", ")
}

#[test]
fn setters_write_through_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Door".to_string());
    let frame = scene.add_entity("Frame".to_string());
    scene.world.set_joint(id, Some(JointComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Joint.SetKind({id}, 'hinge')
             Joint.SetKind({id}, 'slider')
             Joint.SetConnectedBody({id}, {frame})
             Joint.SetAnchor({id}, -0.5, 0, 0)
             Joint.SetAxis({id}, 0, 4, 0)
             Joint.SetUseLimits({id}, true)
             Joint.SetLimits({id}, 110, -500)
             Joint.SetSwingLimit({id}, 30)
             Joint.SetBreakForce({id}, -3)
             Joint.SetBreakTorque({id}, 75)
             Joint.SetAutoConfigureConnectedAnchor({id}, false)
             Joint.SetConnectedAnchor({id}, 1, 2, 3)
             Joint.SetEnableCollision({id}, true)"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "return Joint.GetKind({id}), Joint.GetConnectedBody({id}), Joint.GetUseLimits({id}),
                 Joint.GetBreakForce({id}), Joint.GetKind(99), Joint.GetConnectedBody(99)"
            ),
        );
        let expected =
            format!(r#"String("Hinge"), Integer({frame}), Boolean(true), Number(0), Nil, Nil"#);
        assert_eq!(got, expected);
        let axis = eval(lua, &format!("return Joint.GetAxis({id})"));
        assert_eq!(axis, "Number(0), Number(1), Number(0)");
        let limits = eval(lua, &format!("return Joint.GetLimits({id})"));
        assert_eq!(limits, "Number(-179), Number(110)");
    });
    let j = scene.borrow().world.joint(id).map(|j| j.clone()).unwrap();
    assert_eq!(j.anchor, Vec3::new(-0.5, 0.0, 0.0));
    assert_eq!(j.connected_anchor, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!((j.swing_limit, j.break_torque), (30.0, 75.0));
    assert!(j.enable_collision && !j.auto_configure_connected_anchor);
}

#[test]
fn a_joint_survives_save_and_load() {
    let mut scene = Scene::new();
    let body = scene.add_entity("Arm".to_string());
    let other = scene.add_entity("Torso".to_string());
    let joint = JointComponent {
        kind: JointKind::Ball,
        connected_body: Some(other),
        anchor: Vec3::new(0.0, 0.4, 0.0),
        use_limits: true,
        limits: Vec2::new(-20.0, 60.0),
        break_force: 900.0,
        ..Default::default()
    };
    scene.world.set_joint(body, Some(joint.clone()));
    let path = crate::temp::dir()
        .join(format!("rusty_joint_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded.world.joint(body).map(|j| j.clone()), Some(joint));
}

#[test]
fn the_connected_body_follows_a_prefab_stamp() {
    let mut scene = Scene::new();
    let outside = scene.add_entity("World anchor".to_string());
    let root = scene.add_entity("Torso".to_string());
    let arm = scene.add_entity("Arm".to_string());
    scene.set_parent(arm, Some(root)).unwrap();
    let to = |body| JointComponent {
        connected_body: Some(body),
        ..Default::default()
    };
    scene.world.set_joint(arm, Some(to(root)));
    scene.world.set_joint(root, Some(to(outside)));
    let prefab = extract_prefab(&scene, root).unwrap();
    let joint_of = |e: &Entity| e.joint.as_ref().and_then(|j| j.connected_body);
    assert_eq!(
        joint_of(&prefab.entities[1]),
        Some(0),
        "the root's local id"
    );
    assert_eq!(
        joint_of(&prefab.entities[0]),
        None,
        "outside: joined to the world"
    );

    let mut other = Scene::new();
    other.add_entity("Offset".to_string());
    let stamped = instantiate_prefab(&mut other, &prefab, None);
    let stamped_arm = other.world.children(stamped)[0];
    let got = other
        .world
        .joint(stamped_arm)
        .and_then(|j| j.connected_body);
    assert_eq!(got, Some(stamped), "re-pointed at the stamped root");
    assert!(Entity::is_ref_pointer("/joint/connected_body"));
    assert!(!Entity::is_ref_pointer("/joint/break_force"));
}
