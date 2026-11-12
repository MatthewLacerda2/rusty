//! A scenario's `Scene.GetScript` handle writes a live script's field and calls
//! its function in the gameplay VM; the demo notes both and its replay makes
//! them again, bit for bit (#2120). Unrecorded, each one stops the bot chasing
//! the player only on the recording, and the replay diverges.

use rusty::dev::demo::{self, Demo, Destination, Replayer, Verdict};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

const BOT: &str = "assets/scripts/bot.lua";
const NO_FILE: &str = "Play started on a scene with no file";

/// The bot's health set to zero through the handle: a dead bot stops chasing.
const SET: &str = r#"
Harness.Step(2)
local bot = Scene.GetScript(Scene.FindEntityByName("Enemy_1"), "bot")
bot.health = 0
Harness.Step(238)
"#;

/// The bot's own damage handler called through the handle.
const CALL: &str = r#"
Harness.Step(2)
local id = Scene.FindEntityByName("Enemy_1")
local bot = Scene.GetScript(id, "bot")
bot.OnTakeDamage(id, 100, { type = "bullet" })
Harness.Expect(bot.health <= 0, "the handler ran")
Harness.Step(238)
"#;

/// A table with mixed keys crosses the handle but cannot be noted.
const MIXED: &str = r#"
Harness.Step(2)
local bot = Scene.GetScript(Scene.FindEntityByName("Enemy_1"), "bot")
bot.notes = { 1, k = 2 }
Harness.Step(1)
"#;

fn round_trip(name: &str, code: &str) -> Verdict {
    let dir = crate::temp::dir().join(name);
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), BOT);
    let destination = Destination::File(path.clone());
    demo::record(&mut harness.world.borrow_mut(), destination);
    assert!(scenario::run_code(harness, code, "=record").unwrap().passed);
    let demo = Demo::read(&path).unwrap();
    let ticks = u32::try_from(demo.ticks).unwrap();
    let harness = Harness::new(dir.join("replay"), BOT);
    demo::prepare(&mut harness.world.borrow_mut(), demo).unwrap();
    harness.step(ticks);
    let mut world = harness.world.borrow_mut();
    let replayer = world.tap_mut::<Replayer>().unwrap();
    assert!(replayer.finished());
    replayer.verdict().clone()
}

#[test]
fn a_field_written_through_a_handle_replays() {
    let verdict = round_trip("demo_handle_set", SET);
    assert_eq!((verdict.diverged, verdict.checked), (None, 3));
    assert_eq!(verdict.taints, [(0, NO_FILE.to_string())]);
}

#[test]
fn a_function_called_through_a_handle_replays() {
    let verdict = round_trip("demo_handle_call", CALL);
    assert_eq!((verdict.diverged, verdict.checked), (None, 3));
    assert_eq!(verdict.taints, [(0, NO_FILE.to_string())]);
}

#[test]
fn a_value_the_demo_cannot_carry_taints_it() {
    let verdict = round_trip("demo_handle_mixed", MIXED);
    let mixed = "a handle on script 'bot' (Set) was given a table with mixed keys";
    assert_eq!(
        verdict.taints,
        [(0, NO_FILE.to_string()), (2, mixed.to_string())]
    );
}
