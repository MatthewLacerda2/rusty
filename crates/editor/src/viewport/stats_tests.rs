use super::*;

fn clock() -> Clock {
    Clock {
        fps: 30.0,
        frame_ms: 33.0,
    }
}

/// The table's averages, the heaviest GPU passes first, and counts grouped.
#[test]
fn lines_read_the_windowed_table() {
    let mut stats = FrameStats::default();
    stats.drawn_frames = 2;
    for (key, v) in [("fps", 60.0), ("wall_ms", 16.0), ("frame_ms", 0.25)] {
        stats.record(key, v);
    }
    stats.record("renderer_ms", 4.0);
    stats.record("gpu_ms", 12.5);
    stats.record("draw_calls", 1234.0);
    stats.record("triangles", 2_500_000.0);
    for (pass, ms) in [
        ("ui", 0.1),
        ("forward", 8.0),
        ("ssr", 3.0),
        ("taa", 1.0),
        ("ssao", 0.4),
    ] {
        stats.record_gpu_pass(pass, ms);
    }
    let got = lines(Some(&stats), clock());
    assert_eq!(
        got[0], "60 FPS   frame 16.00 ms",
        "the table, not the clock"
    );
    assert_eq!(got[1], "Sim (per tick)   0.25 ms");
    assert_eq!(got[3], "GPU   12.50 ms");
    assert_eq!(got[4], "Draw calls 1,234   Tris 2,500,000");
    let passes: Vec<_> = got[5..]
        .iter()
        .map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(
        passes,
        [Some("forward"), Some("ssr"), Some("taa"), Some("ssao")]
    );
}

/// No GPU time is said in words, never shown as 0; no table at all (a build
/// without `dev`) shows the window's clock.
#[test]
fn a_missing_gpu_time_or_table_is_said() {
    let mut stats = FrameStats::default();
    assert!(lines(Some(&stats), clock())[3].contains("nothing drawn yet"));
    stats.drawn_frames = 1;
    let got = lines(Some(&stats), clock());
    assert!(got[3].contains("no timestamps"), "{got:?}");
    assert!(
        got[0].starts_with("30 FPS"),
        "falls back to the clock: {got:?}"
    );
    let bare = lines(None, clock());
    assert_eq!(bare[0], "30 FPS   33.00 ms");
    assert!(bare[1].contains("dev build"));
}
