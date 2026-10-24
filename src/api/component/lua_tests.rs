//! `Component.Get` / `Set` / `Fields` from Lua (#1369).

use std::cell::RefCell;

use mlua::Lua;

use super::kinds::KINDS;
use super::register;
use crate::scene::authoring::{add_component, ComponentKind};
use crate::scene::Scene;

/// Run `chunk` with `Component` registered over a scene holding entity `id` (a
/// Rigidbody and a Light), returning what it evaluates to.
fn run<R: mlua::FromLuaMulti>(chunk: &str) -> (Scene, mlua::Result<R>) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Body".to_string());
    add_component(&mut scene, id, ComponentKind::RigidBody);
    add_component(&mut scene, id, ComponentKind::Light);
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.globals().set("id", id).unwrap();
    let out = lua.scope(|s| {
        register(&lua, s, &scene).unwrap();
        lua.load(chunk).eval::<R>()
    });
    (scene.into_inner(), out)
}

fn error(chunk: &str) -> String {
    run::<()>(chunk).1.unwrap_err().to_string()
}

#[test]
fn get_reads_fields_and_set_writes_through_the_op() {
    let (scene, out) = run::<(f32, bool, String)>(
        r#"
        Component.Set(id, "Rigidbody", { mass = 5000, use_gravity = false,
            collision_detection = "continuous" })
        local rb = Component.Get(id, "rigidbody")
        return rb.mass, rb.use_gravity, rb.collision_detection
        "#,
    );
    // The op's clamp, the enum read in any case and written back by its name.
    assert_eq!(out.unwrap(), (1000.0, false, "Continuous".to_string()));
    assert!(scene.world.rigidbody(1).is_some_and(|rb| !rb.use_gravity));
}

#[test]
fn get_is_nil_without_the_component() {
    let (_, out) = run::<bool>(r#"return Component.Get(id, "Camera") == nil"#);
    assert!(out.unwrap());
}

#[test]
fn strict_keys_name_what_is_valid() {
    let e = error(r#"Component.Get(id, "Rigidbdy")"#);
    assert!(
        e.contains("unknown component 'Rigidbdy'") && e.contains("RigidBody"),
        "{e}"
    );
    let e = error(r#"Component.Set(id, "RigidBody", { mas = 1 })"#);
    assert!(
        e.contains("unknown field 'mas'") && e.contains("mass"),
        "{e}"
    );
    let e = error(r#"Component.Set(id, "Transform", { position = {1, 2, 3} })"#);
    assert!(
        e.contains("read-only") && e.contains("Transform.SetLocalPosition"),
        "{e}"
    );
    let e = error(r#"Component.Set(id, "Camera", { fov = 60 })"#);
    assert!(e.contains("has no Camera"), "{e}");
}

#[test]
fn a_failed_set_writes_nothing() {
    let (scene, out) = run::<()>(
        r#"Component.Set(id, "RigidBody", { mass = 3, collision_detection = "Sideways" })"#,
    );
    let e = out.unwrap_err().to_string();
    assert!(e.contains("collision_detection"), "{e}");
    let mass = scene.world.rigidbody(1).map(|rb| rb.mass);
    assert_ne!(mass, Some(3.0), "the good field was not written alone");
}

#[test]
fn fields_lists_types_defaults_and_access() {
    let (_, out) = run::<(String, String, f32, bool, bool, String)>(
        r#"
        local rows = Component.Fields("RigidBody")
        local mass
        for _, row in ipairs(rows) do if row.name == "mass" then mass = row end end
        local cam = Component.Fields("Camera")
        local active = cam[1]
        return rows[1].name, mass.type, mass.default, mass.writable,
            active.writable, active.read_only
        "#,
    );
    let (first, ty, default, writable, cam_writable, why) = out.unwrap();
    assert_eq!(
        (first.as_str(), ty.as_str(), writable),
        ("active", "number", true)
    );
    assert_eq!(default, 1.0);
    assert!(!cam_writable && why.contains("#1368"), "{why}");
}

#[test]
fn every_component_round_trips_get_into_set_through_lua() {
    // What `Get` returns, `Set` takes back: read-only fields dropped, empty lists
    // crossing as `{}`, enums as their names.
    let mut scene = Scene::new();
    let id = scene.add_entity("Dressed".to_string());
    for add in ComponentKind::ALL {
        add_component(&mut scene, id, *add);
    }
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.globals().set("id", id).unwrap();
    lua.scope(|s| {
        register(&lua, s, &scene).unwrap();
        for kind in KINDS.iter().filter(|k| k.add.is_some()) {
            let before = (kind.read)(&scene.borrow().world, id);
            let chunk = format!(
                r#"local c = Component.Get(id, "{0}")
                for _, row in ipairs(Component.Fields("{0}")) do
                    if not row.writable then c[row.name] = nil end
                end
                Component.Set(id, "{0}", c)"#,
                kind.name
            );
            lua.load(&chunk)
                .exec()
                .unwrap_or_else(|e| panic!("{}: {e}", kind.name));
            let after = (kind.read)(&scene.borrow().world, id);
            assert_eq!(before, after, "{}: Get then Set changed it", kind.name);
        }
        Ok(())
    })
    .unwrap();
}
