//! `Scene.SetParent` / `ClearParent` move the subtree's collider bounds (#1067).
//!
//! A re-parent keeps the child's local transform, so the child and everything under
//! it jump in world space. `Physics.GetBounds` reads the cached world AABB, so it is
//! the query that went stale before the fix. Gated on `dev` (the headless session).

use rusty::dev::session::Session;

fn ok(session: &Session, line: &str) -> String {
    session
        .eval(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

/// The x of the centre of `id`'s cached collider bounds.
fn bounds_centre_x(session: &Session, id: &str) -> f32 {
    let x = ok(
        session,
        &format!("local f, a, _, _, b = Physics.GetBounds({id}); assert(f); return (a + b) / 2"),
    );
    x.parse().unwrap_or_else(|_| panic!("not a number: {x}"))
}

/// A Box with a collider at local `x`.
fn crate_at(session: &Session, x: f32) -> String {
    let id = ok(session, "Scene.CreateEntity(\"Crate\", \"Box\")");
    ok(session, &format!("Scene.AddComponent({id}, \"Collider\")"));
    ok(session, &format!("Transform.SetPosition({id}, {x}, 0, 0)"));
    id
}

fn assert_near(got: f32, want: f32, what: &str) {
    assert!(
        (got - want).abs() < 1e-3,
        "{what}: want x={want}, got {got}"
    );
}

#[test]
fn set_and_clear_parent_move_the_whole_subtrees_bounds() {
    let session = Session::new("").expect("session boots in edit mode");
    let parent = ok(&session, "Scene.CreateEntity(\"Parent\")");
    ok(
        &session,
        &format!("Transform.SetPosition({parent}, 10, 0, 0)"),
    );
    let child = crate_at(&session, 1.0);
    let grandchild = crate_at(&session, 2.0);
    ok(&session, &format!("Scene.SetParent({grandchild}, {child})"));
    assert_near(bounds_centre_x(&session, &grandchild), 3.0, "under child");

    ok(&session, &format!("Scene.SetParent({child}, {parent})"));
    assert_near(
        bounds_centre_x(&session, &child),
        11.0,
        "child under parent",
    );
    assert_near(bounds_centre_x(&session, &grandchild), 13.0, "grandchild");

    ok(&session, &format!("Scene.ClearParent({child})"));
    assert_near(bounds_centre_x(&session, &child), 1.0, "child cleared");
    assert_near(bounds_centre_x(&session, &grandchild), 3.0, "grandchild");
}
