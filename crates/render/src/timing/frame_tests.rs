use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_a_frame_counts_and_an_aside_render_does_not() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let empty = Scene::new();
    let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    assert_eq!(r.frame_cost().frames, 0, "nothing drawn yet");
    r.render(&mut view, &scene, &camera, &out, false);
    let (cost, counters) = (r.frame_cost(), r.frame_counters);
    assert_eq!(cost.frames, 1);
    assert!(cost.cpu_ms > 0.0, "{cost:?}");
    assert!(counters.draw_calls > 0, "the box drew");
    r.aside(|r| r.render(&mut view, &empty, &camera, &out, false));
    assert_eq!(r.frame_cost(), cost, "an aside render is not a frame");
    assert_eq!(r.frame_counters, counters, "nor are its counters");
    r.capture_static_cubemap(&scene, glam::Vec3::ZERO, 4);
    assert_eq!(r.frame_cost(), cost, "a capture's faces are not frames");
    assert_eq!(r.frame_counters, counters, "nor are their counters");
    r.render(&mut view, &empty, &camera, &out, false);
    assert_eq!(r.frame_cost().frames, 2, "the next frame counts again");
}

/// Stages are timed only when asked for, and then they hold the frame's whole
/// CPU time, the scene pass among them.
#[test]
fn gpu_frame_stages_are_off_by_default_and_sum_to_the_frame_when_on() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    r.render(&mut view, &scene, &camera, &out, false);
    assert!(r.frame_stages().is_empty(), "off unless asked for");
    r.time_stages(true);
    r.render(&mut view, &scene, &camera, &out, false);
    let stages = r.frame_stages();
    let names: Vec<_> = stages.iter().map(|(s, _)| *s).collect();
    for want in ["setup", "extract", "shadows", "scene", "post_fx", "finish"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    let sum: f64 = stages.iter().map(|(_, ms)| ms).sum();
    let whole = r.frame_cost().cpu_ms;
    assert!(sum <= whole && sum > whole * 0.97, "{sum} of {whole}");
}

#[test]
fn gpu_an_aside_render_inside_a_frame_keeps_the_frames_gpu_call_counts() {
    use crate::gpu::ops::{self, CountedQueue as _};
    let Some(mut r) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Box", Some(Primitive::Box));
    let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 4.0), -90.0, 0.0);
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let out = view.color_target_view().unwrap();
    r.begin_frame_clocks();
    r.queue.counted_submit(std::iter::empty());
    let drew = r.aside(|r| {
        r.render(&mut view, &scene, &camera, &out, false);
        r.frame_counters.ops
    });
    assert!(
        drew.render_passes > 0,
        "the aside render counted its own: {drew:?}"
    );
    r.queue.counted_submit(std::iter::empty());
    let frame = ops::take();
    assert_eq!(frame.submits, 2, "the frame's two calls, whole: {frame:?}");
    assert_eq!(frame.render_passes, 0, "and none of the aside's: {frame:?}");
}
