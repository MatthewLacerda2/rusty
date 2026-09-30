//! Input through the real sim tick (#416): edges published by `GameWorld::tick`,
//! the Play cursor default, and injected input replaying identically headless.

use rusty::dev::console::evaluate_line;
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

/// Evaluate `line` in the harness world's gameplay VM.
fn eval(h: &Harness, line: &str) -> String {
    let world = h.world.borrow();
    evaluate_line(world.script_manager(), world.console(), line).expect("line evaluates")
}

#[test]
fn edges_are_per_sim_tick_and_play_locks_the_cursor() {
    let h = Harness::new(std::env::temp_dir().join("rusty_input_edges"), "");
    h.step(1); // enter Play
    assert_eq!(eval(&h, "return Input.IsCursorLocked()"), "true");

    h.world.borrow().input().borrow_mut().press("Mouse0");
    h.world.borrow().input().borrow_mut().release("Mouse0");
    h.step(1);
    let tap = "return Input.GetKeyDown('Mouse0'), Input.GetKeyUp('Mouse0')";
    assert_eq!(
        eval(&h, tap),
        "true, true",
        "a tap between ticks is not lost"
    );
    h.step(1);
    assert_eq!(eval(&h, tap), "false, false", "edges last one tick");
}

/// Walk forward while swinging the mouse and typing, logging the Player's pose.
const SCENARIO: &str = r#"
local player = Scene.FindEntityByName("Player")
Input.MoveMouse(100, 50)
Input.Press("W")
for _ = 1, 30 do
    Input.AddMouseDelta(3, -1)
    Input.Scroll(0.5)
    Input.TypeText("x")
    Harness.Step(1)
end
Input.Release("W")
Input.Press("Mouse0"); Harness.Step(2); Input.Release("Mouse0"); Harness.Step(2)
local x, y, z = Transform.GetPosition(player)
Harness.Log(string.format("player %.6f %.6f %.6f", x, y, z))
Harness.Expect(Harness.Frame() == 34, "stepped 34 ticks")
"#;

#[test]
fn injected_input_replays_identically() {
    let dir = std::env::temp_dir().join("rusty_input_replay");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scenario.lua");
    std::fs::write(&path, SCENARIO).unwrap();
    let run = |out: &str| {
        let report = scenario::run(&path, &dir.join(out)).expect("scenario runs");
        assert!(report.passed, "scenario expectations pass");
        std::fs::read_to_string(report.results_path).unwrap()
    };
    assert_eq!(run("a"), run("b"), "same injected input, same run");
}
