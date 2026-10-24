//! What a scenario may not do through the one API surface (#1384): spend on a
//! provider (a scenario is a test, and a test never calls a vendor), or hand the
//! timer scheduler a function from its own VM, which the scheduler never runs.

use super::run;

/// Before the first `Harness.Step` the world is not in Play yet, and the
/// provider verbs are refused all the same.
const PROVIDERS: &str = r#"
local why = "headless run"
local function refused(f, ...)
    local ok, err = pcall(f, ...)
    return not ok and tostring(err):find(why, 1, true) ~= nil
end
Harness.Expect(refused(Sfx.Generate, { prompt = "a door creak", seconds = 1 }), "Sfx.Generate")
Harness.Expect(refused(Speech.Generate, { text = "hello", voice = "v" }), "Speech.Generate")
Harness.Expect(refused(Speech.Voices), "Speech.Voices")
Harness.Expect(refused(Speech.FindVoice, "Rachel"), "Speech.FindVoice")
Harness.Expect(refused(Speech.VoiceInfo, "v"), "Speech.VoiceInfo")
Harness.Step(1)
why = "during Play"
Harness.Expect(refused(Sfx.Generate, { prompt = "a door creak", seconds = 1 }), "and after it")
"#;

#[test]
fn provider_verbs_are_refused_from_the_start() {
    let (passed, json) = run("rusty_scenario_providers", PROVIDERS);
    assert!(passed, "scenario expectations pass: {json}");
}

const TIMERS: &str = r#"
local player = Scene.FindEntityByName("Player")
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
