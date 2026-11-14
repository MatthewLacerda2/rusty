//! One teardown order (#1298): a destroy and a scene unload both fire `OnDisable`
//! then `OnDestroy`, each pass children before parents, each hook exactly once,
//! and an entity already inactive gets only its `OnDestroy`.

use super::destroy_subtree::{destroy, scripted};
use super::timers::tick;
use crate::scripting::manager::ScriptManager;

/// A scripted parent with a scripted child under it (the child's id is higher).
fn family(name: &str) -> (ScriptManager, u32, u32) {
    scripted(name, |scene, parent| {
        let child = scene.add_entity("Child".to_string());
        scene.set_parent(child, Some(parent)).unwrap();
        child
    })
}

#[test]
fn an_inactive_child_gets_on_destroy_but_not_on_disable() {
    let (mut m, parent, child) = family("destroy_inactive");
    m.cells.scene.borrow_mut().world.set_active(child, false);
    tick(&mut m, 1); // the falling edge fires the child's OnDisable now
    let log: String = m.eval("__log").unwrap();
    assert!(log.starts_with(&format!("D{child}")), "{log}");
    m.eval("__log = ''").unwrap();
    destroy(&mut m, parent);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{parent}X{child}X{parent}"),
        "no second OnDisable for the already inactive child"
    );
}

#[test]
fn a_parent_and_child_queued_together_fire_each_hook_once() {
    let (mut m, parent, child) = family("destroy_both");
    {
        let mut scene = m.cells.scene.borrow_mut();
        scene.request_destroy(child);
        scene.request_destroy(parent);
        scene.request_destroy(child);
    }
    m.apply_pending_destroys();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{child}D{parent}X{child}X{parent}")
    );
}

#[test]
fn an_unload_tears_down_in_the_order_a_destroy_does() {
    let (mut m, parent, child) = family("unload_order");
    assert!(
        parent < child,
        "ascending ids alone would put the parent first"
    );
    m.unload_entities(&[parent, child]);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{child}D{parent}X{child}X{parent}"),
        "children before parents, as a destroy"
    );
}
