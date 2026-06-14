//! Bot-player end-to-end session (issue #10).
//! Gated on `dev`: a no-op under a plain `cargo test`, real under `--features dev`.
#![cfg(feature = "dev")]

use std::path::Path;

use rusty::dev::botplayer::{attach_player_bot, PLAYER_BOT_SCRIPT};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

#[test]
fn attach_player_bot_tags_the_player() {
    let h = Harness::new(std::env::temp_dir().join("rusty_bot_attach"), "");
    let attached = attach_player_bot(&mut h.world.borrow().scene.borrow_mut(), PLAYER_BOT_SCRIPT);
    assert!(attached, "demo scene has a Player to tag");
}

#[test]
fn play_session_scenario_wins_and_is_deterministic() {
    let scenario = Path::new("project/scenarios/play_session.lua");
    let run = |dir: &str| {
        let out = std::env::temp_dir().join(dir);
        let report = scenario::run(scenario, &out).expect("scenario runs");
        let json = std::fs::read_to_string(report.results_path).expect("results written");
        (report.passed, json)
    };
    let (passed_a, json_a) = run("rusty_bot_session_a");
    let (passed_b, json_b) = run("rusty_bot_session_b");

    assert!(passed_a, "bot-played session should pass its expectations");
    assert!(passed_b, "bot-played session should pass on the rerun too");
    assert_eq!(json_a, json_b, "headless replay must be byte-identical");
}
