//! GPU occlusion culling at its edges (#1963): an empty scene, a lone box, and a
//! history that grows past its room.

use glam::Vec3;

use super::cull::CameraCull;
use super::gpu_tests::{drawn, offscreen, shot, words, H, W};
use rusty_scene::lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_an_empty_scene_and_a_lone_box_cull_nothing() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let cam = Camera::new(Vec3::new(0.0, 1.0, 6.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    let (_, c) = shot(&mut r, &mut view, &Scene::new(), &cam);
    assert_eq!(c.hiz_tested, 0, "nothing to test");
    let mut one = Scene::new();
    block(&mut one, "Box", Vec3::splat(-0.5), Vec3::splat(0.5));
    for _ in 0..3 {
        shot(&mut r, &mut view, &one, &cam);
        assert_eq!(drawn(&r, &view), [1], "a lone box hides nothing");
    }
}

#[test]
fn gpu_a_growing_history_keeps_its_verdicts_and_marks_only_new_slots_drawn() {
    let Some(r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut cam = CameraCull::new(&r.device);
    cam.hold(&r.device, &r.queue, 4);
    // The cull hid slot 1 last frame.
    r.queue
        .write_buffer(cam.history.buffer(), 4, bytemuck::bytes_of(&0u32));
    let before = cam.history.buffer().clone();
    cam.hold(&r.device, &r.queue, 3000); // past the buffer's 1024 words
    assert_ne!(cam.history.buffer(), &before, "it grew");
    let history = words(&r, cam.history.buffer());
    assert_eq!(cam.slots(), 4096);
    assert_eq!(history[1], 0, "the hidden verdict survives the growth");
    let drawn = |s: &[u32]| s.iter().all(|&w| w == 1);
    assert!(
        drawn(&history[..1]) && drawn(&history[2..4096]),
        "the rest draw"
    );
}
