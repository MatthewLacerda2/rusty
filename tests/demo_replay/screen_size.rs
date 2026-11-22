//! The screen size is a sim input: headless it never comes from the machine's
//! saved video settings, and a replay reads the screen its recording's sim read.

use rusty::core::settings::video::{VideoSettings, VIDEO_NAMESPACE};
use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;
use rusty::dev::scenario;

const SIZE: &str = "local w, h = UI.GetScreenSize() return w .. 'x' .. h";

fn screen_size(harness: &Harness) -> String {
    read(harness, SIZE)
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

/// Through the camera's reader: a ray's direction depends on the screen's aspect.
const RAY: &str = "local _, _, _, x, y, z = Camera.ScreenToWorldRay(100, 100) \
                   return string.format('%.4f %.4f %.4f', x, y, z)";

fn read(harness: &Harness, code: &str) -> String {
    let world = harness.world.borrow();
    let read = (world.resources.script_manager).eval(&format!("(function() {code} end)()"));
    read.unwrap()
}

/// As a script or the console does it in edit mode, before Play.
fn set_resolution(harness: &Harness, width: u32, height: u32) {
    let mut world = harness.world.borrow_mut();
    world.init_edit_runtime().unwrap();
    let line = format!("Video.SetResolution({width}, {height})");
    world.resources.script_manager.eval(&line).unwrap();
}

#[test]
fn a_replay_reads_the_screen_its_recording_read_not_its_hosts() {
    let dir = crate::temp::dir().join("demo_screen_size");
    let path = dir.join("run.demo");
    let expected = Harness::new(dir.join("expected"), "");
    set_resolution(&expected, 800, 600);
    expected.step(1);
    let ray = read(&expected, RAY);

    // A game's own write, never saved: the header holds the live settings.
    let harness = Harness::new(dir.join("record"), "");
    set_resolution(&harness, 800, 600);
    demo::record(
        &mut harness.world.borrow_mut(),
        Destination::File(path.clone()),
    );
    let report = scenario::run_code(harness, "Harness.Step(3)", "=record").unwrap();
    assert!(report.passed);
    let demo = Demo::read(&path).unwrap();
    assert_eq!(demo.header.size, None, "recorded headless: no Game view");

    // Another machine's saved settings and live video, and an editor's panel size.
    let replay = Harness::new(dir.join("replay"), "");
    save_video(&replay, 333, 222);
    set_resolution(&replay, 333, 222);
    let panel = replay.world.borrow().resources.screen.clone();
    panel.borrow_mut().set_game_view(640, 480);
    demo::prepare(&mut replay.world.borrow_mut(), demo).unwrap();
    replay.step(1);
    assert_eq!(screen_size(&replay), "800.0x600.0", "UI.GetScreenSize");
    assert_eq!(read(&replay, RAY), ray, "Camera.ScreenToWorldRay");
}
