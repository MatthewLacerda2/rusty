//! Structural-authoring API end-to-end (issue #181).
//!
//! Drives the headless edit-mode session's command channel through the live Lua
//! evaluator: create an entity, add + configure a component, parent it, and save —
//! then reload the saved file into a fresh scene and assert it round-trips.
//! Gated on `dev`: a no-op under a plain `cargo test`, real under `--features dev`.
#![cfg(feature = "dev")]

use rusty::dev::session::Session;
use rusty::scene::Scene;

/// Unwrap a session eval, surfacing the Lua error on failure.
fn ok(session: &Session, line: &str) -> String {
    session
        .eval(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

#[test]
fn create_configure_parent_save_round_trips() {
    let session = Session::new("").expect("session boots in edit mode");

    // Create a parent (bare) and a child (a Box primitive) via the API.
    let parent_id = ok(&session, "Scene.CreateEntity(\"Parent\")");
    let child_id = ok(&session, "Scene.CreateEntity(\"Crate\", \"Box\")");
    assert_ne!(parent_id, "0");
    assert_ne!(child_id, "0");

    // Add + configure a component on the child, then parent it. `Light.SetIntensity`
    // writes the light's scalar intensity.
    ok(
        &session,
        &format!("Scene.AddComponent({child_id}, \"Light\")"),
    );
    ok(&session, &format!("Light.SetIntensity({child_id}, 42)"));
    ok(
        &session,
        &format!("Scene.SetParent({child_id}, {parent_id})"),
    );

    // Save to a fresh temp file via the explicit-path form.
    let out = std::env::temp_dir().join("rusty_structural_authoring.scene");
    let out_str = out.to_str().unwrap().replace('\\', "/");
    let saved = ok(&session, &format!("Scene.Save(\"{out_str}\")"));
    assert_eq!(saved, out_str, "Save returns the written path");
    assert!(out.exists(), "scene file was written");

    // Reload into a brand-new scene and assert the authored structure survived.
    let mut reloaded = Scene::new();
    reloaded
        .load_from_file(&out_str)
        .expect("saved scene loads back");

    let child: u32 = child_id.parse().unwrap();
    let parent: u32 = parent_id.parse().unwrap();
    let e = reloaded.get_entity(child).expect("child entity present");
    assert_eq!(e.parent_id, Some(parent), "parenting round-trips");
    let mesh = e.mesh.as_ref().expect("box primitive mesh round-trips");
    assert_eq!(mesh.primitive_type, "Box");
    assert!(!mesh.vertices.is_empty(), "primitive geometry rehydrated");
    let light = e.light.as_ref().expect("added Light round-trips");
    assert_eq!(light.intensity, 42.0, "configured value round-trips");
}

#[test]
fn destroy_really_removes_while_deactivate_keeps() {
    let session = Session::new("").expect("session boots");

    let keep = ok(&session, "Scene.CreateEntity(\"Keep\")");
    let gone = ok(&session, "Scene.CreateEntity(\"Gone\")");

    ok(&session, &format!("Scene.Deactivate({keep})"));
    let destroyed = ok(&session, &format!("Scene.DestroyEntity({gone})"));
    assert_eq!(
        destroyed, "true",
        "destroying an existing entity reports true"
    );

    let keep_id: u32 = keep.parse().unwrap();
    let gone_id: u32 = gone.parse().unwrap();
    let scene = session.world().scene().borrow();
    assert!(
        scene.get_entity(keep_id).is_some_and(|e| !e.active),
        "Deactivate keeps the entity but clears its active flag"
    );
    assert!(
        scene.get_entity(gone_id).is_none(),
        "DestroyEntity actually removes the entity"
    );
}

#[test]
fn remove_component_round_trips_through_api() {
    let session = Session::new("").expect("session boots");
    let id = ok(&session, "Scene.CreateEntity(\"Mob\")");
    ok(&session, &format!("Scene.AddComponent({id}, \"Collider\")"));
    let removed = ok(
        &session,
        &format!("Scene.RemoveComponent({id}, \"Collider\")"),
    );
    assert_eq!(removed, "true");

    let eid: u32 = id.parse().unwrap();
    let scene = session.world().scene().borrow();
    assert!(scene.get_entity(eid).unwrap().collider.is_none());
}
