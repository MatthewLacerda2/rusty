//! The transition callbacks' (#323) scope: one entity's edges never touch a
//! sibling, the unified active gate (a disabled entity gets no `OnTrigger`), and
//! deterministic dispatch order. Tags as in `transitions.rs`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::components::ScriptComponent;
use crate::physics::TriggerEvents;

use super::lifecycle::{manager_with_entity, write_script};
use super::transitions::{DT, LOGGER};

#[test]
fn destroying_one_entity_leaves_a_sibling_running() {
    // Destroying A mid-play must touch only A. apply_pending_destroys's two
    // `!ids.contains` predicates keep every *surviving* entity's script instance
    // (entity_scripts) AND its load record (load_attempted). Drop the `!` on the
    // first and B's instance is evicted — B goes silent, no more Update. Drop it on
    // the second and B is dropped from load_attempted, so the next spawn scan
    // reloads it — B re-runs Awake/OnEnable/Start on a fresh instance mid-play. A
    // lone-entity destroy triangulates neither; it needs a surviving sibling, and
    // the sibling must carry a real ScriptComponent so the reload path is live.
    let (mut m, a) = manager_with_entity();
    let b = m.scene.borrow_mut().add_entity("B".to_string());
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("trans_sibling", LOGGER);
    for id in [a, b] {
        m.scene
            .borrow_mut()
            .world
            .scripts_mut(id)
            .unwrap()
            .push(ScriptComponent {
                path: path.clone(),
                ..Default::default()
            });
    }
    // The spawn scan owns loading (sets load_attempted); both come alive and run one
    // active tick together.
    m.load_new_scripts();
    m.init_scripts();
    m.update_scripts(DT);
    // Destroy A; isolate the log so only the teardown + B's next tick show up.
    m.eval("__log = ''").unwrap();
    m.scene.borrow_mut().request_destroy(a);
    m.apply_pending_destroys();
    // A full next tick: the spawn scan (the reload path), init, then update.
    m.load_new_scripts();
    m.init_scripts();
    m.update_scripts(DT);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{a}X{a}U{b}"),
        "A tears down (OnDisable→OnDestroy); B keeps updating with no re-Awake/Start \
         — its instance is neither evicted nor reloaded"
    );
    assert!(
        !m.scene.borrow().world.contains(a),
        "the destroyed entity is gone"
    );
    assert!(m.scene.borrow().world.contains(b), "the sibling survives");
}

#[test]
fn disabled_entity_receives_no_on_trigger() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "trans_trigger",
        "_G.__t = false\nreturn { OnTrigger = function(id, other) _G.__t = true end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // awoken while active
    m.scene.borrow_mut().world.set_active(id, false);
    m.dispatch_trigger_events(TriggerEvents {
        stayed: vec![(id, 99)],
        ..Default::default()
    });
    assert_eq!(
        m.eval("__t").unwrap(),
        "false",
        "a disabled entity gets no OnTrigger — the same active gate as Update"
    );
}

/// Loads an `OnEnable`-tagged script at three slots (two entities, one carrying
/// two scripts), inits once, and returns the concatenated tags — the observable
/// enable dispatch order. A pure function of its inputs, so replays match.
fn on_enable_order_log() -> String {
    let (mut m, a) = manager_with_entity();
    let b = m.scene.borrow_mut().add_entity("E2".to_string());
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let tag = |name: &str, t: &str| {
        write_script(
            name,
            &format!("_G.__log = _G.__log or ''\nreturn {{ OnEnable = function(id) __log = __log .. '{t}' end }}"),
        )
    };
    m.load_entity_script(a, 0, &tag("tr_p", "P"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(a, 1, &tag("tr_q", "Q"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(b, 0, &tag("tr_r", "R"), &BTreeMap::new())
        .unwrap();
    m.init_scripts();
    m.eval("__log").unwrap()
}

#[test]
fn on_enable_dispatches_in_ascending_key_order_and_replays_identically() {
    let first = on_enable_order_log();
    assert_eq!(first, "PQR", "ascending (entity, script index) order");
    assert_eq!(
        first,
        on_enable_order_log(),
        "dispatch must be deterministic"
    );
}
