//! What entering Play would wipe is refused before the first `Harness.Step`
//! (#1384): the harness enters Play on that step, which clears the timers and
//! restarts `Random`. After the step, the same calls take effect.

use super::run;

const SCRIPT: &str = r#"
local player = Scene.FindEntityByName("Player")
local function refused(verb, f, ...)
    local ok, err = pcall(f, ...)
    return not ok and tostring(err):find(verb .. " before the first Harness.Step", 1, true) ~= nil
end
Harness.Expect(refused("Timer.Invoke", Timer.Invoke, player, "Start", 0.05), "an invoke")
Harness.Expect(refused("Timer.InvokeRepeating", Timer.InvokeRepeating, player, "Start", 0, 1),
    "a repeat")
Harness.Expect(refused("Timer.StartCoroutine", Timer.StartCoroutine, player, "Start"), "a coroutine")
Harness.Expect(refused("Tween.To", Tween.To, player, "Transform.position", { 1, 2, 3 }, 0.5),
    "a tween")
Harness.Expect(refused("Tween.Sequence", Tween.Sequence, player), "a sequence")
Harness.Expect(refused("Random.SetSeed", Random.SetSeed, 7), "a seed")

Harness.Step(1)
-- `Start` zeroes the controller's vertical speed: set one only it would undo.
local pc = Scene.GetScript(player, "player_controller")
pc.vy = 1000
local h = Timer.Invoke(player, "Start", 0.05)
Harness.Expect(Timer.IsPending(h), "pending once Play has begun")
Harness.Step(10)
Harness.Expect(not Timer.IsPending(h), "done after its delay")
Harness.Expect(pc.vy < 100, "the invoke fired: Start reset the speed (" .. pc.vy .. ")")

Random.SetSeed(7); local a = Random.Value()
Random.SetSeed(7)
Harness.Expect(Random.Value() == a, "a seed set in Play holds")
"#;

#[test]
fn what_play_would_wipe_is_refused_until_it_has_begun() {
    let (passed, json) = run("rusty_scenario_before_play", SCRIPT);
    assert!(passed, "scenario expectations pass: {json}");
}
