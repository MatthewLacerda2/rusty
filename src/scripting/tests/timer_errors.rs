//! How `Timer.*` (#444) fails: bad invoke targets are refused or logged, and a
//! failing coroutine, invoke or predicate surfaces on the console.

use super::coroutines::{co, log};
use super::timers::{counted, rig, tick};

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

#[test]
fn a_failing_invoke_keeps_repeating_and_a_failing_predicate_retires() {
    let pred = "coroutine.yield(Timer.WaitUntil(function() error('pred') end))";
    let (mut m, id) = rig("co_errs", &co(pred));
    let rep = "function() __log = __log .. 'r' error('x') end";
    m.eval(&format!("Timer.InvokeRepeating({id}, {rep}, 0, 1 / 60)"))
        .unwrap();
    tick(&mut m, 4);
    assert_eq!(log(&m), "rrr", "an erroring repeat keeps its schedule");
    let logs: Vec<String> = m
        .console
        .borrow()
        .messages
        .iter()
        .map(|l| l.0.clone())
        .collect();
    assert!(
        logs.iter()
            .any(|l| l.contains("Coroutine") && l.contains("pred")),
        "{logs:?}"
    );
    assert!(
        logs.iter().any(|l| l.contains("Invoke on entity")),
        "{logs:?}"
    );
    assert_eq!(m.eval(&format!("Timer.IsInvoking({id})")).unwrap(), "true");
}
