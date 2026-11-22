use super::*;
use crate::stats::{record_window, WindowFrame};
use rusty_script::scripting::LogLevel::Warning;

#[test]
fn a_frame_of_a_world_is_written_with_what_was_logged() {
    let dir = rusty_test_support::temp::dir();
    let session = crate::session::Session::new("").unwrap();
    let game = session.world();
    let mut log = FrameLog::start(&dir, LogSettings::default()).unwrap();
    let frame = WindowFrame {
        last_wall_ms: 16.6,
        fps: Some(60.0),
        ..WindowFrame::default()
    };
    let run = |log: &mut FrameLog, msg: &str| {
        let stats = game.script_manager().stats_cell();
        let mut stats = stats.borrow_mut();
        stats.record("frame_ms", 2.0);
        // Either side of `SYSTEM_MS` (0.25), so only `step_physics` is written.
        stats.record_system("step_physics", 1.5);
        stats.record_system("tiny", 0.01);
        stats.end_tick();
        record_window(&mut stats, &frame, true);
        drop(stats);
        game.console().borrow_mut().info(msg.into());
        log.frame(game, &frame, false);
    };
    run(&mut log, "logged before the first frame");
    run(&mut log, "hello");
    let off = [("console".to_string(), false)];
    let app = &game.resources.application;
    app.borrow_mut().set_log_channels(&off).unwrap();
    run(&mut log, "console is off");
    drop(log);
    let got = read(&dir, &LogQuery::default(), now_ms());
    let chs: Vec<&str> = got
        .records
        .iter()
        .map(|r| r["ch"].as_str().unwrap())
        .collect();
    assert_eq!(chs, ["frames", "console", "frames", "console", "frames"]);
    let f = &got.records[0];
    assert_eq!(
        (f["ms"].as_f64(), f["fps"].as_f64()),
        (Some(16.6), Some(60.0))
    );
    assert_eq!(
        (f["ticks"].as_u64(), f["sim_ms"].as_f64()),
        (Some(1), Some(2.0))
    );
    assert_eq!(f["systems"], serde_json::json!({ "step_physics": 1.5 }));
    assert!(f["cam"].is_array() && f["fwd"].is_array() && f["scene"].is_string());
    assert_eq!(got.records[1]["msg"], "logged before the first frame");
    assert_eq!(got.records[3]["msg"], "hello");
}

#[test]
fn a_host_off_or_a_budget_of_zero_starts_no_log() {
    let dir = rusty_test_support::temp::dir();
    let none = LogSettings {
        budget_mb: 0,
        ..LogSettings::default()
    };
    assert!(FrameLog::start(&dir, none).is_none());
    assert!(FrameLog::for_host(LogHost::Player, LogSettings::default()).is_none());
}

#[test]
fn a_play_load_is_one_record_and_an_edit_load_none() {
    use rusty_core::input::InputState;
    use rusty_navigation::NavigationGraph;
    use rusty_scene::Scene;
    use std::{cell::RefCell, rc::Rc};
    let dir = rusty_test_support::temp::dir();
    let level = rusty_core::paths::to_slash(&dir.join("level.scene"));
    let mut lvl = Scene::new();
    lvl.add_entity("Level".to_string());
    rusty_edit::save_to_file(&lvl, &level).unwrap();
    let nav = NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0);
    let console = rusty_script::scripting::ConsoleLogs::new();
    let mut gw = GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(console)),
    );
    crate::stats::install(&mut gw);
    let mut log = FrameLog::start(&dir.join("logs"), LogSettings::default()).unwrap();
    let frame = WindowFrame::default();
    rusty_edit::load_from_file(&mut gw.scene().borrow_mut(), &level).unwrap();
    gw.edit_frame();
    log.frame(&gw, &frame, false);
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    log.frame(&gw, &frame, false);
    gw.scene().borrow_mut().request_load(level);
    gw.tick(1.0 / 60.0);
    log.frame(&gw, &frame, false);
    drop(log);
    let loads = LogQuery {
        channel: Some("loads".into()),
        ..LogQuery::default()
    };
    let got = read(&dir.join("logs"), &loads, now_ms());
    let ticks: Vec<u64> = got
        .records
        .iter()
        .map(|r| r["tick"].as_u64().unwrap())
        .collect();
    assert_eq!(ticks, [2], "the Play load alone, on its own tick");
}

#[test]
fn every_console_line_is_written_and_a_repeat_each_frame() {
    let dir = rusty_test_support::temp::dir();
    let session = crate::session::Session::new("").unwrap();
    let game = session.world();
    let (mut log, frame) = (
        FrameLog::start(&dir, Default::default()).unwrap(),
        WindowFrame::default(),
    );
    log.frame(game, &frame, false);
    // Past the console's 100-line buffer, under the file feed's 1024-line cap: none is dropped.
    for i in 0..150 {
        game.console().borrow_mut().info(format!("load step {i}"));
    }
    for _ in 0..3 {
        let msg = "[Physics] skipped".to_string();
        game.console().borrow_mut().repeated(msg, Warning, 2);
        log.frame(game, &frame, false);
    }
    drop(log);
    let got = read(&dir, &LogQuery::default(), now_ms()).records;
    let warns: Vec<_> = got.iter().filter(|r| r["level"] == "warn").collect();
    let infos = got.iter().filter(|r| r["level"] == "info").count();
    assert_eq!(infos, 150, "past the buffer");
    assert_eq!(warns.len(), 3, "one per frame that saw it");
    assert!(warns.iter().all(|w| w["times"] == 2), "{warns:?}");
}
