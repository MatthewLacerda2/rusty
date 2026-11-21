//! What a scenario may not do through the one API surface (#1384): hand the timer
//! scheduler a function from its own VM, which the scheduler never runs.

use super::run;

const TIMERS: &str = r#"
local player = Scene.FindEntityByName("Player")
Harness.Step(1)
local function refused(verb, f, ...)
    local ok, err = pcall(f, ...)
    return not ok and tostring(err):find(verb, 1, true) ~= nil
end
local noop = function() end
Harness.Expect(refused("Timer.StartCoroutine", Timer.StartCoroutine, player, noop), "a coroutine")
Harness.Expect(refused("Timer.Invoke", Timer.Invoke, player, noop, 0.1), "an invoke")
Harness.Expect(refused("Timer.InvokeRepeating", Timer.InvokeRepeating, player, noop, 0, 1), "a repeat")
Harness.Expect(refused("Tween on_complete", Tween.To, player, "Transform.position", { 1, 2, 3 }, 0.5,
    { on_complete = noop }), "a tween callback")
Harness.Expect(pcall(Timer.Invoke, player, "Update", 0.1), "a named invoke resolves in the gameplay VM")
Harness.Step(3)
"#;

#[test]
fn a_scenario_function_is_refused_by_the_scheduler() {
    let (passed, json) = run("rusty_scenario_timers", TIMERS);
    assert!(passed, "scenario expectations pass: {json}");
}
