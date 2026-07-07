//! Tests for the `LateUpdate` post-physics callback (#324): it rides the shared
//! dispatch core with the same scaled `dt` as `Update`, fires only on started &
//! active instances, dispatches in ascending `(entity, script index)` order, and
//! — the point of the hook — observes the transform *after* the tick's physics
//! step where `Update` observed it before. Physics is simulated here by mutating
//! the scene between the two dispatch calls; the end-to-end schedule ordering (a
//! real rigidbody moving between `Update` and `LateUpdate`) is proven at the
//! `GameWorld::tick` level in `app/late_update_tests.rs`.

use std::collections::BTreeMap;

use glam::Vec3;

use super::tests_lifecycle::{manager_with_entity, write_script};

/// `LateUpdate` is handed the same scaled `dt` the tick gives `Update`.
#[test]
fn late_update_passes_delta_time() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&std::rc::Rc::new(std::cell::RefCell::new(None)))
        .unwrap();
    let path = write_script(
        "late_dt",
        "_G.__late_dt = 0\nreturn { LateUpdate = function(id, dt) _G.__late_dt = dt end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // LateUpdate, like Update, is gated on Start having run
    m.late_update_scripts(0.25);
    let got: f64 = m.eval("__late_dt").unwrap().parse().unwrap();
    assert!((got - 0.25).abs() < 1e-6, "dt should be 0.25, got {got}");
}

/// The headline contract: in one tick `Update` sees the pre-physics transform and
/// `LateUpdate` sees the post-physics one. Physics is stood in for by moving the
/// entity between the two dispatch calls (the order the play schedule enforces).
#[test]
fn late_update_observes_post_physics_transform_same_tick() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&std::rc::Rc::new(std::cell::RefCell::new(None)))
        .unwrap();
    let path = write_script(
        "late_post_physics",
        "_G.__u = -1\n_G.__l = -1\nreturn {\n\
         Update = function(id) _G.__u = ({Transform.GetPosition(id)})[1] end,\n\
         LateUpdate = function(id) _G.__l = ({Transform.GetPosition(id)})[1] end,\n}",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts();

    // Update reads the pre-physics x (0.0, the default).
    m.update_scripts(1.0 / 60.0);
    // "Physics" resolves this tick: the body ends up at x = 5.
    m.scene.borrow_mut().world.transform_mut(id).unwrap().position = Vec3::new(5.0, 0.0, 0.0);
    // LateUpdate reads the post-physics x, same tick.
    m.late_update_scripts(1.0 / 60.0);

    let u: f64 = m.eval("__u").unwrap().parse().unwrap();
    let l: f64 = m.eval("__l").unwrap().parse().unwrap();
    assert!((u - 0.0).abs() < 1e-6, "Update saw pre-physics x, got {u}");
    assert!(
        (l - 5.0).abs() < 1e-6,
        "LateUpdate saw post-physics x, got {l}"
    );
}

/// An inactive entity's `LateUpdate` does not fire (the same active filter as
/// `Update`), even for an instance that already started while it was active.
#[test]
fn inactive_entity_late_update_does_not_fire() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&std::rc::Rc::new(std::cell::RefCell::new(None)))
        .unwrap();
    let path = write_script(
        "late_inactive",
        "_G.__fired = false\nreturn { LateUpdate = function(id) _G.__fired = true end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // starts while active
    m.scene.borrow_mut().world.set_active(id, false);
    m.late_update_scripts(1.0 / 60.0);
    assert_eq!(
        m.eval("__fired").unwrap(),
        "false",
        "LateUpdate must skip an inactive entity"
    );
}

/// Builds a fresh runtime, loads a tagged `LateUpdate` at each `(entity, index)`
/// slot, inits, fires one `LateUpdate`, and returns the concatenated tag log —
/// the observable dispatch order. A pure function of its inputs, so two runs at
/// fixed dt must return byte-identical strings (the determinism contract).
fn late_update_order_log() -> String {
    let (mut m, a) = manager_with_entity();
    let b = m.scene.borrow_mut().add_entity("E2".to_string());
    m.init_runtime(&std::rc::Rc::new(std::cell::RefCell::new(None)))
        .unwrap();
    let tag = |name: &str, t: &str| {
        write_script(
            name,
            &format!("_G.__log = _G.__log or ''\nreturn {{ LateUpdate = function(id) __log = __log .. '{t}' end }}"),
        )
    };
    // Two scripts on `a` (index 0, 1) plus one on `b`: proves entity order AND
    // script-index order in a single string.
    m.load_entity_script(a, 0, &tag("late_p", "P"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(a, 1, &tag("late_q", "Q"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(b, 0, &tag("late_r", "R"), &BTreeMap::new())
        .unwrap();
    m.init_scripts();
    m.late_update_scripts(1.0 / 60.0);
    m.eval("__log").unwrap()
}

#[test]
fn late_update_dispatches_in_ascending_key_order_and_replays_identically() {
    let first = late_update_order_log();
    assert_eq!(first, "PQR", "ascending (entity, script index) order");
    // Byte-identical replay at the same fixed dt.
    assert_eq!(
        first,
        late_update_order_log(),
        "dispatch must be deterministic"
    );
}
