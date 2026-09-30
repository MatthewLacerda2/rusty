//! Tests for `Timer.Invoke` & co. (#444): invokes fire on the tick their scaled
//! time elapses (never the tick they were scheduled), repeat without drift, cancel
//! by handle / name, and die with a deactivated or destroyed entity.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::manager::ScriptManager;
use super::tests_lifecycle::{manager_with_entity, write_script};

pub(super) const DT: f32 = 1.0 / 60.0;

/// A live runtime with `code` attached to one entity. Scripts that want the tick
/// number read `__t`, which the prelude's `Update` counter maintains.
pub(super) fn rig(name: &str, code: &str) -> (ScriptManager, u32) {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(name, code);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    (m, id)
}

/// One play tick the way the schedule runs it: advance the clock, init, `Update`,
/// then the timer phase on the tick's scaled dt.
pub(super) fn tick(m: &mut ScriptManager, n: u32) {
    for _ in 0..n {
        let dt = {
            let mut t = m.time.borrow_mut();
            t.advance(DT);
            t.delta_time
        };
        m.init_scripts();
        m.update_scripts(dt);
        m.tick_timers(dt);
    }
}

/// A script with a tick counter (`__t`), a log, and `body` as its `Start`.
pub(super) fn counted(body: &str) -> String {
    format!(
        "_G.__t = 0\n_G.__log = ''\nlocal M = {{}}\n\
         function M.Update(id) __t = __t + 1 end\n\
         function M.Beat(id) __log = __log .. __t .. ',' end\n\
         function M.Start(id) {body} end\nreturn M"
    )
}

#[test]
fn invoke_fires_once_on_the_tick_its_delay_elapses() {
    // Scheduled on tick 1; 0.05 s is three 1/60 steps, counted from tick 2.
    let (mut m, _) = rig("timer_once", &counted("Timer.Invoke(id, 'Beat', 0.05)"));
    tick(&mut m, 10);
    assert_eq!(m.eval("__log").unwrap(), "4,");
    assert!(m.timers.borrow().is_empty(), "a one-shot retires");
}

#[test]
fn zero_delay_invoke_waits_for_the_next_tick() {
    let (mut m, _) = rig("timer_zero", &counted("Timer.Invoke(id, M.Beat, 0)"));
    tick(&mut m, 1);
    assert_eq!(m.eval("__log").unwrap(), "");
    tick(&mut m, 1);
    assert_eq!(m.eval("__log").unwrap(), "2,");
}

#[test]
fn invoke_repeating_keeps_its_interval_until_cancelled_by_name() {
    let code = counted("Timer.InvokeRepeating(id, 'Beat', 0, 2 / 60)");
    let (mut m, id) = rig("timer_repeat", &code);
    tick(&mut m, 7);
    // Drift-free: deadlines at 0, 2, 4, 6 steps after scheduling (tick 1),
    // the first held to the next tick by the one-tick rule.
    assert_eq!(m.eval("__log").unwrap(), "2,3,5,7,");
    assert_eq!(
        m.eval(&format!("Timer.IsInvoking({id}, 'Beat')")).unwrap(),
        "true"
    );
    m.eval(&format!("Timer.CancelInvoke({id}, 'Beat')"))
        .unwrap();
    tick(&mut m, 5);
    assert_eq!(m.eval("__log").unwrap(), "2,3,5,7,");
    assert_eq!(m.eval(&format!("Timer.IsInvoking({id})")).unwrap(), "false");
}

#[test]
fn a_handle_cancels_exactly_its_own_timer() {
    let code = counted("_G.__a = Timer.Invoke(id, 'Beat', 0.05) Timer.Invoke(id, 'Beat', 0.1)");
    let (mut m, _) = rig("timer_handle", &code);
    tick(&mut m, 1);
    assert_eq!(m.eval("Timer.IsPending(__a)").unwrap(), "true");
    assert_eq!(m.eval("Timer.Cancel(__a)").unwrap(), "true");
    assert_eq!(m.eval("Timer.Cancel(__a)").unwrap(), "false");
    tick(&mut m, 10);
    assert_eq!(m.eval("__log").unwrap(), "7,", "only the 0.1 s timer fires");
}

#[test]
fn time_scale_zero_freezes_invokes() {
    let (mut m, _) = rig("timer_scale", &counted("Timer.Invoke(id, 'Beat', 0.05)"));
    tick(&mut m, 1);
    m.time.borrow_mut().set_time_scale(0.0);
    tick(&mut m, 10);
    assert_eq!(m.eval("__log").unwrap(), "");
    m.time.borrow_mut().set_time_scale(1.0);
    tick(&mut m, 3);
    assert_eq!(m.eval("__log").unwrap(), "14,");
}

#[test]
fn deactivate_and_destroy_drop_pending_timers() {
    let (mut m, id) = rig("timer_life", &counted("Timer.Invoke(id, 'Beat', 0.05)"));
    tick(&mut m, 1);
    m.scene.borrow_mut().world.set_active(id, false);
    tick(&mut m, 1);
    assert!(m.timers.borrow().is_empty(), "deactivation stops timers");
    m.scene.borrow_mut().world.set_active(id, true);
    tick(&mut m, 1);
    m.eval(&format!("Timer.Invoke({id}, 'Beat', 0)")).unwrap();
    m.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert!(m.timers.borrow().is_empty(), "destroy stops timers");
    assert_eq!(m.eval("__log").unwrap(), "");
}

#[test]
fn bad_targets_are_refused_or_logged() {
    let (mut m, id) = rig("timer_bad", &counted("Timer.Invoke(id, 'Nope', 0)"));
    tick(&mut m, 2);
    let logs = m
        .console
        .borrow()
        .messages
        .iter()
        .map(|l| l.0.clone())
        .collect::<Vec<_>>();
    assert!(
        logs.iter()
            .any(|l| l.contains("Invoke on entity") && l.contains("Nope")),
        "{logs:?}"
    );
    assert!(
        m.eval("Timer.Invoke(999, 'Beat', 0)").is_err(),
        "missing entity"
    );
    assert!(m
        .eval(&format!("Timer.InvokeRepeating({id}, 'Beat', 0, 0)"))
        .is_err());
    m.scene.borrow_mut().world.set_active(id, false);
    assert!(
        m.eval(&format!("Timer.Invoke({id}, 'Beat', 0)")).is_err(),
        "inactive"
    );
}
