//! Tests for `Timer.StartCoroutine` and the yield instructions (#444): a coroutine
//! runs to its first yield at once, resumes on the tick its wait ends, honours
//! (or, realtime, ignores) the time scale, stops with its entity, surfaces its
//! errors on the console, and replays identically.

use super::tests_timers::{counted, rig, tick};

/// Start a coroutine logging `__t` around the waits in `body`.
fn co(body: &str) -> String {
    counted(&format!(
        "Timer.StartCoroutine(id, function(id)\n{body}\nend)"
    ))
}

fn log(m: &super::manager::ScriptManager) -> String {
    m.eval("__log").unwrap()
}

#[test]
fn coroutine_runs_to_first_yield_then_resumes_on_schedule() {
    let body = "M.Beat(id) coroutine.yield() M.Beat(id)\n\
                coroutine.yield(Timer.WaitForSeconds(0.05)) M.Beat(id)\n\
                coroutine.yield(Timer.WaitForFixedUpdate()) M.Beat(id)";
    let (mut m, _) = rig("co_sched", &co(body));
    tick(&mut m, 10);
    // Start runs before tick 1's Update, so the first beat reads 0.
    assert_eq!(log(&m), "0,2,5,6,");
    assert!(m.timers.borrow().is_empty(), "a finished coroutine retires");
}

#[test]
fn time_scale_zero_freezes_scaled_waits_but_not_realtime() {
    let body = "Timer.StartCoroutine(id, function() \
                coroutine.yield(Timer.WaitForSecondsRealtime(0.05)) __log = __log .. 'R' .. __t .. ',' end)\n\
                coroutine.yield(Timer.WaitForSeconds(0.05)) __log = __log .. 'S' .. __t .. ','";
    let (mut m, _) = rig("co_scale", &co(body));
    m.time.borrow_mut().set_time_scale(0.0);
    tick(&mut m, 10);
    assert_eq!(log(&m), "R4,");
    m.time.borrow_mut().set_time_scale(1.0);
    tick(&mut m, 3);
    assert_eq!(log(&m), "R4,S13,");
}

#[test]
fn wait_until_resumes_on_the_first_truthy_tick() {
    let body = "coroutine.yield(Timer.WaitUntil(function() return __t >= 5 end)) M.Beat(id)";
    let (mut m, _) = rig("co_until", &co(body));
    tick(&mut m, 8);
    assert_eq!(log(&m), "5,");
}

#[test]
fn deactivation_stops_a_coroutine_for_good() {
    let (mut m, id) = rig(
        "co_stop",
        &co("while true do coroutine.yield() M.Beat(id) end"),
    );
    tick(&mut m, 3);
    assert_eq!(log(&m), "2,3,");
    m.scene.borrow_mut().world.set_active(id, false);
    tick(&mut m, 1);
    m.scene.borrow_mut().world.set_active(id, true);
    tick(&mut m, 3);
    assert_eq!(log(&m), "2,3,", "Unity: coroutines stop on deactivate");
    assert!(m.eval(&format!("Timer.StopAllCoroutines({id})")).is_ok());
}

#[test]
fn stop_all_coroutines_and_cancel_end_pending_coroutines() {
    let (mut m, id) = rig(
        "co_cancel",
        &co("while true do coroutine.yield() M.Beat(id) end"),
    );
    tick(&mut m, 2);
    m.eval(&format!("Timer.StopAllCoroutines({id})")).unwrap();
    tick(&mut m, 3);
    assert_eq!(log(&m), "2,");
    assert!(m.timers.borrow().is_empty());
}

#[test]
fn coroutine_errors_surface_on_the_console() {
    let (mut m, id) = rig("co_err", &co("coroutine.yield() error('boom') "));
    m.eval(&format!(
        "Timer.StartCoroutine({id}, function() coroutine.yield(42) end)"
    ))
    .unwrap();
    tick(&mut m, 3);
    let logs: Vec<String> = m
        .console
        .borrow()
        .messages
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(
        logs.iter()
            .any(|l| l.contains("Coroutine on entity") && l.contains("boom")),
        "{logs:?}"
    );
    assert!(
        logs.iter().any(|l| l.contains("unsupported integer")),
        "{logs:?}"
    );
    assert!(m.timers.borrow().is_empty(), "failed coroutines retire");
}

/// The run as a string: two runtimes, same script, same ticks → same bytes.
fn replay() -> String {
    let body = "for i = 1, 4 do coroutine.yield(Timer.WaitForSeconds(i / 60)) M.Beat(id) end";
    let (mut m, id) = rig("co_replay", &co(body));
    m.eval(&format!("Timer.InvokeRepeating({id}, 'Beat', 0.02, 0.03)"))
        .unwrap();
    tick(&mut m, 20);
    log(&m)
}

#[test]
fn two_runs_are_identical() {
    let first = replay();
    assert!(first.len() > 8, "{first}");
    assert_eq!(first, replay());
}
