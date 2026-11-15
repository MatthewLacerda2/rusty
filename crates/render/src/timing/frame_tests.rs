use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

/// A drawn frame moves the count and takes CPU time; a render inside `aside`
/// or a cubemap capture leaves the count, the cost and the counters as the
/// measured frame left them.
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
