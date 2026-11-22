use std::collections::BTreeMap;

use glam::Vec3;

use super::lifecycle::{manager_with_entity, write_script};

#[test]
fn late_update_passes_delta_time() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
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

#[test]
fn late_update_observes_post_physics_transform_same_tick() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script(
        "late_post_physics",
        "_G.__u = -1\n_G.__l = -1\nreturn {\n\
         Update = function(id) _G.__u = ({Transform.GetPosition(id)})[1] end,\n\
         LateUpdate = function(id) _G.__l = ({Transform.GetPosition(id)})[1] end,\n}",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts();

    m.update_scripts(1.0 / 60.0);
    m.cells
        .scene
        .borrow_mut()
        .world
        .transform_mut(id)
        .unwrap()
        .position = Vec3::new(5.0, 0.0, 0.0);
    m.late_update_scripts(1.0 / 60.0);

    let u: f64 = m.eval("__u").unwrap().parse().unwrap();
    let l: f64 = m.eval("__l").unwrap().parse().unwrap();
    assert!((u - 0.0).abs() < 1e-6, "Update saw pre-physics x, got {u}");
    assert!(
        (l - 5.0).abs() < 1e-6,
        "LateUpdate saw post-physics x, got {l}"
    );
}

#[test]
fn inactive_entity_late_update_does_not_fire() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script(
        "late_inactive",
        "_G.__fired = false\nreturn { LateUpdate = function(id) _G.__fired = true end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // starts while active
    m.cells.scene.borrow_mut().world.set_active(id, false);
    m.late_update_scripts(1.0 / 60.0);
    assert_eq!(
        m.eval("__fired").unwrap(),
        "false",
        "LateUpdate must skip an inactive entity"
    );
}

fn late_update_order_log() -> String {
    let (mut m, a) = manager_with_entity();
    let b = m.cells.scene.borrow_mut().add_entity("E2".to_string());
    m.init_runtime().unwrap();
    let tag = |name: &str, t: &str| {
        write_script(
            name,
            &format!("_G.__log = _G.__log or ''\nreturn {{ LateUpdate = function(id) __log = __log .. '{t}' end }}"),
        )
    };
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
    assert_eq!(
        first,
        late_update_order_log(),
        "dispatch must be deterministic"
    );
}
