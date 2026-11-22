//! The screen size is a sim input: headless it never comes from the machine's
//! saved video settings, and a replay reads the size its demo recorded.

use rusty::core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

const SIZE: &str = "local w, h = UI.GetScreenSize() return w .. 'x' .. h";

fn screen_size(harness: &Harness) -> String {
    let world = harness.world.borrow();
    let read = (world.resources.script_manager).eval(&format!("(function() {SIZE} end)()"));
    read.unwrap()
}

fn save_video(harness: &Harness, width: u32, height: u32) {
    let video = VideoSettings {
        width,
        height,
        ..VideoSettings::default()
    };
    let world = harness.world.borrow();
    let mut storage = world.resources.storage.borrow_mut();
    storage.set_namespace(VIDEO_NAMESPACE, video.to_json());
}

#[test]
fn a_headless_run_reads_the_default_size_and_no_save_file() {
    let harness = Harness::new(crate::temp::dir().join("headless_screen_size"), "");
    {
        let world = harness.world.borrow();
        let storage = world.resources.storage.borrow();
        assert_eq!(storage.path(), None, "the harness opened a save file");
        assert_eq!(storage.get_namespace(VIDEO_NAMESPACE), None);
    }
    harness.step(1);
    assert_eq!(screen_size(&harness), "1280.0x720.0");
    let world = harness.world.borrow();
    (world.resources.script_manager)
        .eval("Video.SetResolution(800, 600)")
        .unwrap();
    drop(world);
    assert_eq!(
        screen_size(&harness),
        "800.0x600.0",
        "a script's write is a sim input"
    );
}

#[test]
fn a_headless_replay_reads_the_recorders_video_settings_not_its_hosts() {
    let dir = crate::temp::dir().join("demo_screen_size");
    let path = dir.join("run.demo");
    let harness = Harness::new(dir.join("record"), "");
    save_video(&harness, 800, 600);
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    let report = scenario::run_code(harness, "Harness.Step(3)", "=record").unwrap();
    assert!(report.passed);
    let demo = Demo::read(&path).unwrap();
    assert_eq!(demo.header.size, None, "recorded headless: no Game view");

    let replay = Harness::new(dir.join("replay"), "");
    save_video(&replay, 333, 222);
    *replay
        .world
        .borrow()
        .script_manager()
        .video_cell()
        .borrow_mut() = VideoSettings {
        width: 333,
        height: 222,
        ..VideoSettings::default()
    };
    demo::prepare(&mut replay.world.borrow_mut(), demo).unwrap();
    replay.step(1);
    assert_eq!(screen_size(&replay), "800.0x600.0");
}
