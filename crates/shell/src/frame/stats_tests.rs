use std::time::{Duration, Instant};

use super::*;
use rusty_core::frame_stats::FrameStats;
use rusty_render::{RenderView, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

/// The windowed frame path on a headless renderer: a frame the renderer drew
/// carries its counters and cost, one that drew nothing (or drew only aside, as the
/// Inspector preview does) carries the clock alone, and both reach the stats.
#[test]
fn gpu_a_window_frame_carries_what_the_renderer_drew() {
    let Some(mut r) = rusty_render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    let start = Instant::now();
    let mut clock = FrameClock::new(start);
    clock.tick(start + Duration::from_millis(16));

    let before = r.frame_cost().frames;
    r.render(&mut view, &scene, &camera, &out, false);
    let drew = window_frame(&mut r, &clock, before);
    let render = drew.render.as_ref().expect("the renderer drew");
    assert!(
        render.counters.draw_calls > 0 && render.cpu_ms > 0.0,
        "{render:?}"
    );
    assert!(
        (drew.last_wall_ms - 16.0).abs() < 0.5,
        "{}",
        drew.last_wall_ms
    );

    let before = r.frame_cost().frames;
    assert!(
        window_frame(&mut r, &clock, before).render.is_none(),
        "nothing drawn"
    );
    r.aside(|r| r.render(&mut view, &scene, &camera, &out, false));
    let aside = window_frame(&mut r, &clock, before);
    assert!(aside.render.is_none(), "an aside render is not the frame");

    let mut stats = FrameStats::default();
    rusty_dev::stats::record_window(&mut stats, &drew, true);
    rusty_dev::stats::window_ended(&mut stats, aside.last_wall_ms);
    rusty_dev::stats::record_window(&mut stats, &aside, true);
    assert_eq!(stats.drawn_frames, 1);
    let last = stats.window(1);
    assert!(last.get("wall_ms").is_none(), "it arrives next");
    assert_eq!(stats.window(2).get("wall_ms").map(|s| s.samples), Some(1));
    assert!(
        last.get("draw_calls").is_none(),
        "the last frame drew nothing"
    );
    assert_eq!(
        stats.window(2).get("draw_calls").map(|s| s.samples),
        Some(1)
    );
}

/// A drawn window frame reaches the frame log on disk (#2206) with what the
/// renderer drew, beside the stats, as `measured` hands it over.
#[test]
fn gpu_a_window_frame_reaches_the_frame_log() {
    use rusty_dev::frame_log::{now_ms, read, FrameLog, LogQuery};
    let Some(mut r) = rusty_render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let session = rusty_dev::session::Session::new("").unwrap();
    let game = session.world();
    let dir = rusty_test_support::temp::dir();
    let mut log = FrameLog::start(&dir, Default::default()).unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    let clock = FrameClock::new(Instant::now());
    for _ in 0..2 {
        let before = r.frame_cost().frames;
        r.render(&mut view, &scene, &camera, &out, false);
        let frame = window_frame(&mut r, &clock, before);
        let stats = game.script_manager().stats_cell();
        rusty_dev::stats::record_window(&mut stats.borrow_mut(), &frame, false);
        log.frame(game, &frame, r.times_gpu());
    }
    drop(log);
    let got = read(&dir, &LogQuery::default(), now_ms());
    assert_eq!(got.records.len(), 2, "{:?}", got.records);
    let f = &got.records[1];
    assert!(
        f["draws"].as_u64().unwrap() > 0 && f["cpu_ms"].as_f64().unwrap() > 0.0,
        "{f}"
    );
    assert_eq!(f["play"], false);
}
