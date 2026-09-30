//! `Tween.*` through the scripting runtime (#424): tweens tick in the timer phase
//! on the scaled clock (unless `unscaled`), honour the one-tick rule and their
//! delay, call `on_complete`, and die with a killed handle or a deactivated entity.

use super::manager::ScriptManager;
use super::tests_timers::{counted, rig, tick};

/// `Transform.GetPosition(id)`'s x, rounded to 3 decimals (f32 steps sum inexactly).
fn x(m: &ScriptManager, id: u32) -> String {
    lane(m, "GetPosition", id)
}

fn lane(m: &ScriptManager, getter: &str, id: u32) -> String {
    let code = format!("math.floor(({{Transform.{getter}({id})}})[1] * 1000 + 0.5) / 1000");
    m.eval(&code).unwrap()
}

#[test]
fn a_tween_starts_next_tick_and_lands_on_its_target() {
    // Started on tick 1; 4 steps of 1/60 over 4/60 s → x = 1, 2, 3, 4 on ticks 2..5.
    let body = "Tween.To(id, 'Transform.position', {4, 0, 0}, 4 / 60, \
                { on_complete = function(e) __log = __log .. 'done' .. __t end })";
    let (mut m, id) = rig("tween_basic", &counted(body));
    tick(&mut m, 1);
    assert_eq!(x(&m, id), "0");
    tick(&mut m, 2);
    assert_eq!(x(&m, id), "2");
    tick(&mut m, 2);
    assert_eq!(x(&m, id), "4");
    assert_eq!(m.eval("__log").unwrap(), "done5");
    assert!(m.timers.borrow().is_empty(), "a finished tween retires");
}

#[test]
fn delay_holds_and_from_is_written_at_once() {
    let body = "Tween.To(id, 'Transform.position', {6, 0, 0}, 2 / 60, \
                { from = {2, 0, 0}, delay = 2 / 60 })";
    let (mut m, id) = rig("tween_delay", &counted(body));
    tick(&mut m, 3);
    assert_eq!(x(&m, id), "2", "held at `from` through the delay");
    tick(&mut m, 1);
    assert_eq!(x(&m, id), "4");
}

#[test]
fn time_scale_zero_pauses_a_scaled_tween_but_not_an_unscaled_one() {
    let body = "Time.SetTimeScale(0) \
                Tween.To(id, 'Transform.position', {2, 0, 0}, 2 / 60) \
                Tween.To(id, 'Transform.scale', {3, 3, 3}, 2 / 60, { unscaled = true })";
    let (mut m, id) = rig("tween_unscaled", &counted(body));
    tick(&mut m, 5);
    assert_eq!(x(&m, id), "0");
    assert_eq!(lane(&m, "GetScale", id), "3");
}

#[test]
fn kill_and_deactivate_stop_a_tween() {
    let body = "__h = Tween.To(id, 'Transform.position', {60, 0, 0}, 1, { loops = -1 })";
    let (mut m, id) = rig("tween_kill", &counted(body));
    tick(&mut m, 2);
    assert_eq!(m.eval("Tween.IsPlaying(__h)").unwrap(), "true");
    assert_eq!(m.eval("Tween.Kill(__h)").unwrap(), "true");
    assert_eq!(m.eval("Tween.IsPlaying(__h)").unwrap(), "false");
    let held = x(&m, id);
    tick(&mut m, 2);
    assert_eq!(x(&m, id), held);
    m.eval(&format!(
        "Tween.To({id}, 'Transform.position', {{0, 0, 0}}, 1)"
    ))
    .unwrap();
    m.eval(&format!("Scene.Deactivate({id})")).unwrap();
    tick(&mut m, 1);
    assert!(
        m.timers.borrow().is_empty(),
        "a deactivated entity's tweens stop"
    );
}

#[test]
fn a_sequence_runs_its_items_in_order() {
    let body = "__s = Tween.Sequence{ \
                  {id, 'Transform.position', {2, 0, 0}, 2 / 60}, \
                  1 / 60, \
                  {id, 'Transform.scale', {5, 5, 5}, 1 / 60}, \
                }";
    let (mut m, id) = rig("tween_seq", &counted(body));
    let sx = |m: &ScriptManager| lane(m, "GetScale", id);
    tick(&mut m, 3);
    assert_eq!((x(&m, id), sx(&m)), ("2".into(), "1".into()));
    tick(&mut m, 1);
    assert_eq!(sx(&m), "1", "the gap holds the second item");
    tick(&mut m, 1);
    assert_eq!(sx(&m), "5");
    assert_eq!(m.eval("Tween.IsPlaying(__s)").unwrap(), "false");
}

#[test]
fn bad_arguments_are_errors_not_silent_no_ops() {
    let (m, id) = rig("tween_errors", &counted(""));
    let err = |code: String| m.eval(&code).unwrap_err();
    assert!(
        err(format!("Tween.To({id}, 'Transform.positon', {{0,0,0}}, 1)"))
            .contains("unknown tween property")
    );
    assert!(
        err(format!("Tween.To({id}, 'CanvasGroup.alpha', 1, 1)")).contains("has no CanvasGroup")
    );
    assert!(
        err(format!("Tween.To({id}, 'Transform.position', 1, 1)")).contains("table of 3 numbers")
    );
    assert!(err(format!(
        "Tween.To({id}, 'Transform.position', {{0,0,0}}, 1, {{ ease = 'wobble' }})"
    ))
    .contains("unknown ease"));
    assert!(err(format!(
        "Tween.To({id}, 'Transform.position', {{0,0,0}}, 1, {{ eas = 'quad_in' }})"
    ))
    .contains("unknown tween option"));
    assert!(err(format!(
        "Tween.Sequence{{ {{ {id}, 'Transform.position', {{0,0,0}}, 1, loops = -1 }} }}"
    ))
    .contains("endless"));
}
