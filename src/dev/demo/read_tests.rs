//! Reading a demo back: a tick no line names repeats the last one, but no edge.

use std::io::Write;

use super::super::delta::InputDelta;
use super::super::format::Exact;
use super::*;

fn header() -> Header {
    Header {
        scene: "a.scene".into(),
        engine_commit: "test".into(),
        seed: 1,
        sources: Default::default(),
        settings: Default::default(),
        storage: Value::Null,
        size: None,
        time_scale: Exact(1.0),
        paused: false,
        pending_steps: 0,
        before_play: Vec::new(),
    }
}

#[test]
fn a_clipboard_capture_is_published_on_its_own_tick_only() {
    let pasted = InputFrame {
        held: vec!["W".into()],
        clipboard: Some("copied".into()),
        ..InputFrame::default()
    };
    let entry = Entry {
        t: 0,
        input: InputDelta::between(&InputFrame::default(), &pasted),
        ..Entry::default()
    };
    let end = End {
        end: 3,
        ..End::default()
    };
    let path = crate::test_temp::dir().join("clipboard.demo");
    let mut file = File::create(&path).unwrap();
    for line in [
        serde_json::to_string(&header()).unwrap(),
        serde_json::to_string(&entry).unwrap(),
        serde_json::to_string(&end).unwrap(),
    ] {
        writeln!(file, "{line}").unwrap();
    }
    let ticks: Vec<DemoTick> = Demo::read(&path).unwrap().ticks().collect();
    let clipboards: Vec<_> = ticks.iter().map(|t| t.input.clipboard.clone()).collect();
    assert_eq!(clipboards, [Some("copied".into()), None, None]);
    assert!(
        ticks.iter().all(|t| t.input.held == ["W"]),
        "held keys carry on"
    );
}
