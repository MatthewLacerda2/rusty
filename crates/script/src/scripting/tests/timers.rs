use std::collections::BTreeMap;

use super::lifecycle::{manager_with_entity, write_script};
use crate::scripting::manager::ScriptManager;

pub(super) const DT: f32 = 1.0 / 60.0;

pub(super) fn rig(name: &str, code: &str) -> (ScriptManager, u32) {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script(name, code);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    (m, id)
}

pub(super) fn tick(m: &mut ScriptManager, n: u32) {
    for _ in 0..n {
        let dt = {
            let mut t = m.cells.time.borrow_mut();
            t.advance(DT);
            t.delta_time
        };
        m.init_scripts();
        m.update_scripts(dt);
        m.tick_timers(dt);
    }
}

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
    let (mut m, _) = rig("timer_once", &counted("Timer.Invoke(id, 'Beat', 0.05)"));
    tick(&mut m, 10);
    assert_eq!(m.eval("__log").unwrap(), "4,");
    assert!(m.cells.timers.borrow().is_empty(), "a one-shot retires");
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
    m.cells.time.borrow_mut().set_time_scale(0.0);
    tick(&mut m, 10);
    assert_eq!(m.eval("__log").unwrap(), "");
    m.cells.time.borrow_mut().set_time_scale(1.0);
    tick(&mut m, 3);
    assert_eq!(m.eval("__log").unwrap(), "14,");
}

#[test]
fn deactivate_and_destroy_drop_pending_timers() {
    let (mut m, id) = rig("timer_life", &counted("Timer.Invoke(id, 'Beat', 0.05)"));
    tick(&mut m, 1);
    m.cells.scene.borrow_mut().world.set_active(id, false);
    tick(&mut m, 1);
    assert!(
        m.cells.timers.borrow().is_empty(),
        "deactivation stops timers"
    );
    m.cells.scene.borrow_mut().world.set_active(id, true);
    tick(&mut m, 1);
    m.eval(&format!("Timer.Invoke({id}, 'Beat', 0)")).unwrap();
    m.cells.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert!(m.cells.timers.borrow().is_empty(), "destroy stops timers");
    assert_eq!(m.eval("__log").unwrap(), "");
}
