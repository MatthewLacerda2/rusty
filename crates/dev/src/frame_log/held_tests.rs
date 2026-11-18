//! Frames held until whole (#2206): a frame's own wall time and GPU times, the
//! bound, the order, and what a read sees while they wait.

use super::*;
use crate::frame_log::record::{frame, Place};
use crate::stats::WindowFrame;
use rusty_core::frame_stats::FrameStats;
use rusty_render::GpuPass;

fn record(n: u64, drew: bool) -> FrameRecord {
    let render = drew.then(|| crate::capture::RenderedFrame {
        counters: Default::default(),
        cpu_ms: 1.0,
        stages: Vec::new(),
        gpu: None,
    });
    let window = WindowFrame {
        frame: n,
        render,
        ..WindowFrame::default()
    };
    let place = Place::fixed(n, n);
    frame(place, &window, &FrameStats::default(), Default::default())
}

fn entry(n: u64, wants_gpu: bool) -> Entry {
    Entry::new(
        Some(record(n, true)),
        wants_gpu,
        vec![format!("rest {n}")],
        n,
    )
}

fn gpu(n: u64, ms: f64) -> GpuTimes {
    GpuTimes {
        frame: n,
        passes: vec![(GpuPass::Shadows, ms)],
        span_ms: ms,
        between_submits_ms: 0.0,
    }
}

fn json(line: &str) -> serde_json::Value {
    serde_json::from_str(line).unwrap()
}

/// A hitch's line carries its own 60 ms and its own GPU times, which arrive two
/// frames late; a neighbour's never ride on it.
#[test]
fn a_frame_is_written_with_its_own_times() {
    let mut held = Held::default();
    assert!(held.push(entry(1, true)).is_empty());
    held.ended(60.0); // frame 2 starts: frame 1 took 60 ms
    assert!(
        held.push(entry(2, true)).is_empty(),
        "frame 1 waits on its GPU times"
    );
    held.ended(16.0);
    held.gpu(gpu(1, 9.0));
    let out = held.push(entry(3, true));
    assert_eq!(out.len(), 1, "frame 1 whole, frame 2 not yet");
    let (lines, _) = &out[0];
    assert_eq!(lines[1], "rest 1", "its own lines after it");
    let f = json(&lines[0]);
    assert_eq!(
        (f["frame"].as_u64(), f["ms"].as_f64()),
        (Some(1), Some(60.0))
    );
    assert_eq!(f["gpu_passes"], serde_json::json!({ "shadows": 9.0 }));
    assert!(f.get("unresolved").is_none(), "{f}");
}

/// Past the bound a frame is written as it stands, saying what never came; an
/// adapter without timestamps waits on none; what is held is read as it stands.
#[test]
fn a_frame_waits_at_most_the_bound() {
    let dir = rusty_test_support::temp::dir();
    let shared = register(&dir);
    let mut held = shared.lock().unwrap();
    held.push(entry(1, true));
    for n in 2..=HOLD as u64 {
        held.ended(16.0);
        assert!(held.push(entry(n, true)).is_empty());
    }
    held.ended(16.0);
    let out = held.push(entry(9, true));
    let f = json(&out[0].0[0]);
    assert_eq!(
        (f["frame"].as_u64(), f["unresolved"].clone()),
        (Some(1), serde_json::json!(["gpu"]))
    );
    let mut plain = Held::default();
    plain.push(entry(1, false));
    plain.ended(16.0);
    assert_eq!(
        plain.push(entry(2, false)).len(),
        1,
        "no GPU times to wait on"
    );
    drop(held);
    let seen = lines(&dir);
    assert_eq!(seen.len(), 2 * HOLD as usize, "held frames and their lines");
    let newest = json(&seen[seen.len() - 2]);
    assert_eq!(newest["unresolved"], serde_json::json!(["ms", "gpu"]));
    drop(shared);
    assert!(lines(&dir).is_empty(), "a stopped log holds nothing");
}
