//! How `Timer.*` (#444) fails: bad invoke targets are refused or logged, and a
//! failing coroutine, invoke or predicate surfaces on the console.

use super::coroutines::{co, log};
use super::timers::{counted, rig, tick};

#[test]
fn bad_targets_are_refused_or_logged() {
    let (mut m, id) = rig("timer_bad", &counted("Timer.Invoke(id, 'Nope', 0)"));
    tick(&mut m, 2);
    let logs = m
        .cells
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
    m.cells.scene.borrow_mut().world.set_active(id, false);
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
        .cells
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
    assert!(
        m.cells.timers.borrow().is_empty(),
        "failed coroutines retire"
    );
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
        .cells
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

/// A job whose function belongs to another VM (#1384) never resolves here: it is
/// cancelled with a console error the first time it comes due, not retried.
#[test]
fn a_job_from_another_vm_is_cancelled_once() {
    use crate::scripting::{Target, Wait, Work};
    let (mut m, id) = rig("timer_lost", &counted(""));
    let other = mlua::Lua::new();
    let func = other.create_function(|_, ()| Ok(())).unwrap();
    let key = other.create_registry_value(func).unwrap();
    {
        let mut timers = m.cells.timers.borrow_mut();
        let handle = timers.alloc();
        let work = Work::Invoke {
            target: Target::Func(key),
            interval: Some(0.01),
        };
        timers.insert(handle, id, work, Wait::scaled(0.0));
    }
    tick(&mut m, 4);
    assert!(
        m.cells.timers.borrow().is_empty(),
        "the lost job is cancelled"
    );
    let console = m.cells.console.borrow();
    let lost = console
        .messages
        .iter()
        .filter(|l| l.0.contains("does not resolve in the gameplay VM"))
        .count();
    assert_eq!(lost, 1, "reported once: {:?}", console.messages);
}

/// `Timer` refuses a function from a VM the scheduler does not run (#1384).
#[test]
fn a_function_from_a_non_gameplay_vm_is_refused() {
    let (m, id) = rig("timer_foreign", &counted(""));
    let ctx = m.make_ctx();
    let lua = mlua::Lua::new();
    let err = lua
        .scope(|scope| {
            crate::api::register(&lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            lua.load(format!("Timer.StartCoroutine({id}, function() end)"))
                .exec()
        })
        .unwrap_err();
    assert!(err.to_string().contains("Timer.StartCoroutine"), "{err}");
}
