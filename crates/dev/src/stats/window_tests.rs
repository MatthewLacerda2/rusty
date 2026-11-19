use super::*;
use rusty_render::{GpuPass, GpuTimes, RenderCounters};

fn drawn(draw_calls: u32, gpu: Option<f64>) -> WindowFrame {
    let counters = RenderCounters {
        draw_calls,
        ..Default::default()
    };
    let gpu = gpu.map(|ms| GpuTimes {
        frame: 1,
        passes: vec![(GpuPass::Forward, ms)],
        other_ms: 0.0,
        between_submits_ms: 0.0,
        ..Default::default()
    });
    let render = RenderedFrame {
        counters,
        cpu_ms: 1.5,
        stages: Vec::new(),
        gpu,
    };
    WindowFrame {
        last_wall_ms: 16.0,
        fps: Some(60.0),
        render: Some(render),
        frame: 1,
    }
}

/// A drawn frame records its wall time, frame rate and render, and counts as
/// drawn; the sim's tick count is not the window's.
#[test]
fn a_drawn_frame_records_wall_time_and_its_render() {
    let mut stats = FrameStats::default();
    record_window(&mut stats, &drawn(7, Some(4.0)), true);
    window_ended(&mut stats, 16.0);
    record_window(&mut stats, &drawn(9, None), true);
    assert_eq!((stats.drawn_frames, stats.frames), (2, 0));
    assert_eq!(
        stats.get("wall_ms").unwrap().samples,
        1,
        "the newest frame's wall time arrives with the next"
    );
    assert_eq!(stats.get("fps").unwrap().last, 60.0);
    assert_eq!(stats.get("draw_calls").unwrap().max, 9.0);
    assert_eq!(stats.get("renderer_ms").unwrap().avg, 1.5);
    let gpu = stats.get("gpu_ms").unwrap();
    assert_eq!(
        (gpu.samples, gpu.last),
        (1, 4.0),
        "a frame without timestamps adds none"
    );
    assert_eq!(stats.gpu_passes["forward"].samples, 1);
}

/// A frame that rendered nothing still records the window's clock (its wall time
/// once the next frame starts), and no render key: absent, never zero. A clock
/// with no reading yet records no `fps`.
#[test]
fn a_frame_without_a_render_records_only_the_clock() {
    let mut stats = FrameStats::default();
    let frame = WindowFrame {
        render: None,
        fps: None,
        ..drawn(1, None)
    };
    record_window(&mut stats, &frame, true);
    assert!(stats.get("wall_ms").is_none(), "not ended yet");
    window_ended(&mut stats, 16.0);
    assert_eq!(stats.drawn_frames, 0);
    assert!(stats.get("wall_ms").is_some());
    assert!(stats.get("draw_calls").is_none() && stats.get("gpu_ms").is_none());
    assert!(stats.get("fps").is_none(), "no made-up rate");
}

/// `Debug.Stats{ window = n }`'s argument: absent is the session, a whole number
/// of at least 1 is a window (clamped to what is kept), anything else raises, an
/// unknown key too.
#[test]
fn window_option_takes_whole_frames() {
    let lua = mlua::Lua::new();
    let opts = |src: &str| window_opt(Some(lua.load(src).eval().unwrap()));
    assert_eq!(window_opt(None).unwrap(), None);
    assert_eq!(opts("return {}").unwrap(), None);
    assert_eq!(opts("return { window = 120 }").unwrap(), Some(120));
    assert_eq!(opts("return { window = 1e9 }").unwrap(), Some(RECENT));
    assert_eq!(opts("return { window = math.huge }").unwrap(), Some(RECENT));
    for bad in [
        "{ window = 0 }",
        "{ window = 2.5 }",
        "{ window = 0/0 }",
        "{ window = 'x' }",
        "{ windows = 120 }",
        "{ 120 }",
    ] {
        assert!(opts(&format!("return {bad}")).is_err(), "{bad}");
    }
    let err = opts("return { windows = 120 }").unwrap_err().to_string();
    assert!(
        err.contains("unknown option \"windows\"; expected one of window"),
        "{err}"
    );
}

/// A frame drawn outside Play (after Stop) reaches the window, not the session.
#[test]
fn an_edit_mode_frame_moves_only_the_window() {
    let mut stats = FrameStats::default();
    record_window(&mut stats, &drawn(7, Some(4.0)), true);
    record_window(&mut stats, &drawn(90, Some(8.0)), false);
    assert_eq!(stats.drawn_frames, 1);
    assert_eq!(stats.get("draw_calls").unwrap().max, 7.0);
    assert_eq!(stats.window(1).get("draw_calls").unwrap().last, 90.0);
    assert_eq!(stats.window(1).gpu_passes["forward"].last, 8.0);
}
