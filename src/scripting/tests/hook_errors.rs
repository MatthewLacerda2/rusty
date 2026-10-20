//! Script failures surface on the console instead of stopping play: a script that
//! does not compile, a callback that raises, a coroutine yielding a value that is
//! no instruction, a failing `on_complete`; and a due job its own phase disarmed
//! never runs.

use std::cell::RefCell;
use std::rc::Rc;

use super::lifecycle::{manager_with_entity, write_script};
use super::timers::{counted, rig, tick};
use crate::components::{CanvasGroupComponent, ScriptComponent};
use crate::scripting::manager::ScriptManager;

fn logs(m: &ScriptManager) -> Vec<String> {
    let console = m.console.borrow();
    console.messages.iter().map(|l| l.0.clone()).collect()
}

fn logged(m: &ScriptManager, needle: &str) -> bool {
    logs(m).iter().any(|l| l.contains(needle))
}

#[test]
fn a_script_that_does_not_compile_is_logged_once_and_never_retried() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("err_compile", "return {");
    let slot = ScriptComponent {
        path,
        ..Default::default()
    };
    m.scene
        .borrow_mut()
        .world
        .scripts_mut(id)
        .unwrap()
        .push(slot);
    m.load_new_scripts();
    m.load_new_scripts();
    let errors: Vec<_> = logs(&m)
        .into_iter()
        .filter(|l| l.contains(&format!("Lua compile error (Entity {id})")))
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
}

#[test]
fn a_raising_transition_callback_is_logged_and_play_goes_on() {
    let code = "return { OnDisable = function(id) error('disable failed') end, \
                Update = function(id) _G.__u = (_G.__u or 0) + 1 end }";
    let (mut m, id) = rig("err_disable", code);
    tick(&mut m, 1);
    m.scene.borrow_mut().world.set_active(id, false);
    tick(&mut m, 1);
    m.scene.borrow_mut().world.set_active(id, true);
    tick(&mut m, 1);
    assert!(
        logged(&m, &format!("OnDisable on entity {id} failed")) && logged(&m, "disable failed"),
        "{:?}",
        logs(&m)
    );
    assert_eq!(
        m.eval("__u").unwrap(),
        "2",
        "Update resumes after the error"
    );
}

#[test]
fn a_yield_that_is_no_instruction_retires_the_coroutine() {
    for (name, yielded, error) in [
        ("err_yield_str", "'soon'", "unsupported string"),
        ("err_yield_tbl", "{ 1, 2 }", "unsupported table"),
        (
            "err_yield_arg",
            "{ __wait = 'seconds', arg = 'x' }",
            "must be a number",
        ),
    ] {
        let body = format!("Timer.StartCoroutine(id, function() coroutine.yield({yielded}) end)");
        let (mut m, _) = rig(name, &counted(&body));
        tick(&mut m, 2);
        assert!(logged(&m, error), "{name}: {:?}", logs(&m));
        assert!(
            m.timers.borrow().is_empty(),
            "{name}: a failed coroutine retires"
        );
    }
}

#[test]
fn a_job_cancelled_or_deactivated_earlier_in_the_phase_never_runs() {
    // Both invokes fall due on tick 2; the first runs first and disarms the second.
    let cancel = "__h2 = nil \
                  Timer.Invoke(id, function() Timer.Cancel(__h2) end, 0) \
                  __h2 = Timer.Invoke(id, function() __log = __log .. 'b' end, 0)";
    let (mut m, _) = rig("err_cancelled", &counted(cancel));
    tick(&mut m, 3);
    assert_eq!(m.eval("__log").unwrap(), "");
    let deactivate = "Timer.Invoke(id, function() Scene.SetActive(id, false) end, 0) \
                      Timer.Invoke(id, function() __log = __log .. 'b' end, 0)";
    let (mut m, _) = rig("err_deactivated", &counted(deactivate));
    tick(&mut m, 3);
    assert_eq!(m.eval("__log").unwrap(), "");
    assert!(m.timers.borrow().is_empty());
}

#[test]
fn a_tween_whose_component_went_away_before_its_first_step_ends_silently() {
    let body = "Tween.To(id, 'CanvasGroup.alpha', 0, 1)";
    let (mut m, id) = rig("err_tween_gone", &counted(body));
    let group = Some(CanvasGroupComponent::default());
    m.scene.borrow_mut().world.set_canvas_group(id, group);
    tick(&mut m, 1);
    m.scene.borrow_mut().world.set_canvas_group(id, None);
    tick(&mut m, 2);
    assert!(m.timers.borrow().is_empty(), "the tween retired");
    assert!(!logged(&m, "Tween"), "{:?}", logs(&m));
}

#[test]
fn a_failing_on_complete_is_logged() {
    let body = "Tween.To(id, 'Transform.position', {1, 0, 0}, 1 / 60, \
                { on_complete = function() error('done failed') end })";
    let (mut m, _) = rig("err_tween_done", &counted(body));
    tick(&mut m, 3);
    assert!(
        logged(&m, "Tween on_complete") && logged(&m, "done failed"),
        "{:?}",
        logs(&m)
    );
}
