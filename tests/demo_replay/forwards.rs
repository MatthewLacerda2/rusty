//! A scenario's call that the bridge forwards into the gameplay VM (`Damage.*`)
//! replays forwarded too, so its callbacks fire again (#1929).

use std::path::Path;

use rusty::dev::demo::{self, Demo, Destination, Replayer, Verdict};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

const BOT: &str = "assets/scripts/bot.lua";

/// Kill the bot on the second tick: its `OnTakeDamage` marks it dead, and a dead
/// bot stops chasing the player, so a replay that skips the handler moves it.
const KILL: &str = r#"
Harness.Step(2)
local bot = Scene.FindEntityByName("Enemy_1")
Harness.Expect(Damage.Apply(bot, 100, { type = "bullet" }) == 100, "the bot took it")
Harness.Expect(Scene.GetScript(bot, "bot").health <= 0, "its handler ran")
Harness.Step(238)
"#;

fn record(dir: &Path) -> Demo {
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), BOT);
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    assert!(scenario::run_code(harness, KILL, "=record").unwrap().passed);
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
    let demo = record(&dir);
    let (verdict, health) = replay(&dir, demo);
    assert!(
        health <= 0.0,
        "the replayed handler killed the bot: {health}"
    );
    assert_eq!(
        (verdict.diverged, verdict.checked),
        (None, 3),
        "{:?}",
        verdict.taints
    );
}
