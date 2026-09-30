//! Scope tests for the `Timer` scheduler (#444): cancel / stop / query calls touch
//! only their own entity and their own kind (invoke vs coroutine), deactivating one
//! entity spares every other entity's timers, and a fresh runtime forgets them all.

use std::cell::RefCell;
use std::rc::Rc;

use super::tests_timers::{counted, rig, tick};

const YIELD: &str = "function() while true do coroutine.yield() end end";

#[test]
fn cancels_and_queries_touch_only_their_owner_and_kind() {
    let (m, a) = rig("scope_kind", &counted(""));
    let b = m.scene.borrow_mut().add_entity("B".to_string());
    for id in [a, b] {
        m.eval(&format!("Timer.Invoke({id}, function() end, 1)"))
            .unwrap();
        m.eval(&format!(
            "_G.__co{id} = Timer.StartCoroutine({id}, {YIELD})"
        ))
        .unwrap();
    }
    assert!(!m.timers.borrow().is_empty());
    let q = |m: &super::manager::ScriptManager, lua: String| m.eval(&lua).unwrap();

    m.eval(&format!("Timer.CancelInvoke({a})")).unwrap();
    assert_eq!(q(&m, format!("Timer.IsInvoking({a})")), "false");
    assert_eq!(
        q(&m, format!("Timer.IsInvoking({b})")),
        "true",
        "b's invoke stays"
    );
    assert_eq!(
        q(&m, format!("Timer.IsPending(__co{a})")),
        "true",
        "coroutines stay"
    );

    m.eval(&format!("Timer.StopAllCoroutines({a})")).unwrap();
    assert_eq!(q(&m, format!("Timer.IsPending(__co{a})")), "false");
    assert_eq!(
        q(&m, format!("Timer.IsPending(__co{b})")),
        "true",
        "b's coroutine stays"
    );
    assert_eq!(
        q(&m, format!("Timer.IsInvoking({b})")),
        "true",
        "invokes stay"
    );
}

#[test]
fn deactivating_one_entity_spares_the_others_timers() {
    let (mut m, a) = rig("scope_disable", &counted(""));
    let b = m.scene.borrow_mut().add_entity("B".to_string());
    tick(&mut m, 1); // `a`'s script wakes, so its deactivation is an OnDisable edge
    m.eval(&format!(
        "_G.__b = false Timer.Invoke({b}, function() __b = true end, 0.05)"
    ))
    .unwrap();
    m.scene.borrow_mut().world.set_active(a, false);
    tick(&mut m, 4);
    assert_eq!(m.eval("__b").unwrap(), "true");
}

#[test]
fn a_fresh_runtime_forgets_pending_timers() {
    let (mut m, a) = rig("scope_fresh", &counted(""));
    m.eval(&format!("Timer.Invoke({a}, function() end, 1)"))
        .unwrap();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    assert!(m.timers.borrow().is_empty());
}
