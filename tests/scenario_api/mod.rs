//! The scenario VM gets the one API surface (#1384): the namespaces a gameplay
//! script calls, read-write, plus the scenario-only `Harness`.

use std::path::Path;

use rusty::dev::scenario;

mod crossing;
mod refusals;

/// Write `code` as a scenario and run it on an isolated workspace; its results.
pub(super) fn run(name: &str, code: &str) -> (bool, String) {
    let dir = crate::temp::dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scenario.lua");
    std::fs::write(&path, code).unwrap();
    let report = scenario::run_isolated(&path, &dir.join("out")).expect("scenario runs");
    let json = std::fs::read_to_string(report.results_path).unwrap();
    (report.passed, json)
}

/// The default scene's Player carries `player_controller.lua`, which keeps its
/// vertical speed in `self.vy`.
const SCRIPT: &str = r#"
local player = Scene.FindEntityByName("Player")
Harness.Expect(Scene.GetScript(player, "player_controller") == nil, "no instance before Play")
Harness.Step(2)
local pc = Scene.GetScript(player, "player_controller")
Harness.Expect(pc ~= nil, "the live instance, once Play loaded it")
Harness.Expect(type(pc.vy) == "number", "its fields read by value")
pc.marker = { hits = 3 }
Harness.Expect(Scene.GetScript(player, "player_controller").marker.hits == 3, "writes land")
Harness.Expect(not pcall(function() return pc.Update end), "a function does not cross VMs")
Harness.Expect(Scene.GetScript(player, "nope") == nil, "an unknown script is nil")

Transform.SetPosition(player, 3, 4, 5)
local x, y, z = Transform.GetPosition(player)
Harness.Expect(x == 3 and y == 4 and z == 5, "Transform.SetPosition writes the world")
"#;

#[test]
fn a_scenario_reaches_a_live_script_and_writes_a_transform() {
    let (passed, json) = run("rusty_scenario_api", SCRIPT);
    assert!(passed, "scenario expectations pass: {json}");
}

/// The gameplay VM's own write is visible to the scenario through the handle.
#[test]
fn a_script_handle_reads_the_instance_live() {
    let code = r#"
local player = Scene.FindEntityByName("Player")
Harness.Step(1)
local pc = Scene.GetScript(player, "player_controller")
Harness.StepUntil(function() return CharacterController.IsGrounded(player) end, 120)
Harness.Expect(pc.vy <= 0, "grounded, not rising")
Input.Press("SPACE"); Harness.Step(1); Input.Release("SPACE")
Harness.Expect(pc.vy > 0, "the jump the gameplay VM just set reads through the same handle")
"#;
    let (passed, json) = run("rusty_scenario_live", code);
    assert!(passed, "scenario expectations pass: {json}");
}

#[test]
fn the_smoke_fixture_still_passes() {
    let path = Path::new("tests/fixtures/project/scenarios/smoke.lua");
    let out = crate::temp::dir().join("rusty_scenario_smoke");
    let report = scenario::run_isolated(path, &out).expect("scenario runs");
    let json = std::fs::read_to_string(report.results_path).unwrap();
    assert!(report.passed, "smoke.lua passes on the real API: {json}");
}
