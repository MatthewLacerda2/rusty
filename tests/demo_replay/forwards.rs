//! A scenario's call that the bridge forwards into the gameplay VM (`Damage.*`,
//! `CharacterController.Move`) replays forwarded too, so its callbacks fire
//! again, and one made before the game is up replays as it was made (#1929).

use std::path::Path;

use rusty::dev::demo::{self, Demo, Destination, Replayer, Verdict};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

const BOT: &str = "assets/scripts/bot.lua";
/// The taint every harness demo carries: its scene is built, not loaded.
const NO_FILE: &str = "Play started on a scene with no file";

/// Kill the bot on the second tick: its `OnTakeDamage` marks it dead, and a dead
/// bot stops chasing the player, so a replay that skips the handler moves it.
const KILL: &str = r#"
Harness.Step(2)
local bot = Scene.FindEntityByName("Enemy_1")
Harness.Expect(Damage.Apply(bot, 100, { type = "bullet" }) == 100, "the bot took it")
Harness.Expect(Scene.GetScript(bot, "bot").health <= 0, "its handler ran")
Harness.Step(238)
"#;

/// Move the player before the first step: no gameplay VM yet, so the
/// scenario's own `Move` runs, and the replay must make it the same way.
const MOVE_FIRST: &str = r#"
local player = Scene.FindEntityByName("Player")
CharacterController.Move(player, 1, 0, 0)
Harness.Step(240)
"#;

fn record(dir: &Path, code: &str) -> Demo {
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), BOT);
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    assert!(scenario::run_code(harness, code, "=record").unwrap().passed);
    Demo::read(&path).unwrap()
}

fn replay(dir: &Path, demo: Demo) -> (Verdict, f64) {
    let harness = Harness::new(dir.join("replay"), BOT);
    let ticks = demo.ticks;
    demo::prepare(&mut harness.world.borrow_mut(), demo).unwrap();
    harness.step(u32::try_from(ticks).unwrap());
    let mut world = harness.world.borrow_mut();
    let bot = world
        .scene()
        .borrow()
        .find_entity_by_name("Enemy_1")
        .unwrap();
    let health = world
        .script_manager()
        .with_script(bot, "bot", |_, t| t.get::<f64>("health"))
        .unwrap()
        .expect("the bot's script");
    let replayer = world.tap_mut::<Replayer>().unwrap();
    assert!(replayer.finished());
    (replayer.verdict().clone(), health)
}

#[test]
fn a_forwarded_damage_call_replays_with_its_handler() {
    let dir = crate::temp::dir().join("demo_forwarded_damage");
    let demo = record(&dir, KILL);
    let (verdict, health) = replay(&dir, demo);
    assert!(
        health <= 0.0,
        "the replayed handler killed the bot: {health}"
    );
    assert_eq!((verdict.diverged, verdict.checked), (None, 3));
    // The harness's scene has no file, and `Scene.GetScript`'s handle is the
    // one act the demo cannot carry.
    let handle = "Scene.GetScript returned a handle (userdata), whose calls are not recorded";
    assert_eq!(
        verdict.taints,
        [(0, NO_FILE.to_string()), (2, handle.to_string())]
    );
}

#[test]
fn a_move_before_the_game_is_up_replays_as_it_was_made() {
    let dir = crate::temp::dir().join("demo_forwarded_move_first");
    let demo = record(&dir, MOVE_FIRST);
    let (verdict, _) = replay(&dir, demo);
    assert_eq!((verdict.diverged, verdict.checked), (None, 3));
    assert_eq!(verdict.taints, [(0, NO_FILE.to_string())]);
}
