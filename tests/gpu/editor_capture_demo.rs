//! The editor captured with a demo open (#1598): the timeline strip over a Game
//! view letterboxed to the size the demo plays on, and a demo that does not open is
//! an error, not a capture of something else.

use rusty::app::ProjectSource;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};

use crate::demo_replay::timeline::demo_of_a_saved_scene;

#[test]
fn a_demo_captures_letterboxed_on_its_timeline() {
    let (demo, _) = demo_of_a_saved_scene();
    let out = crate::temp::dir().join("demo.png");
    let mut game = capture::default_world(ProjectSource::defaults());
    // A tall capture: the demo's landscape Game view leaves bars above and below.
    let opts = EditorCaptureOptions {
        width: 600,
        height: 900,
        playing: true,
        ..Default::default()
    };
    if !capture::capture_demo(&mut game, &out, &opts, &demo, 30).expect("captures") {
        return; // no adapter: the skip contract
    }
    let img = image::open(&out).unwrap().to_rgba8();
    let black = |y: u32| (0..600).all(|x| img.get_pixel(x, y).0[..3] == [0, 0, 0]);
    assert!(black(890), "the bar below the letterboxed view is black");
    assert!(!black(450), "the view itself is drawn");
}

#[test]
fn a_demo_that_does_not_open_is_an_error() {
    let mut game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("none.png");
    let opts = EditorCaptureOptions::default();
    let missing = rusty::core::paths::native("no/such.demo");
    let err = capture::capture_demo(&mut game, &out, &opts, &missing, 0).unwrap_err();
    assert!(err.contains("such.demo"), "{err}");
    assert!(!out.exists(), "nothing was written");
}
