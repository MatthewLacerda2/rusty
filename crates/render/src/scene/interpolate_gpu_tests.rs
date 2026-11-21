use glam::Vec3;

use super::fixture::{frame, yard, RES};

#[test]
fn gpu_a_mesh_between_two_ticks_is_drawn_at_its_blend() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(1);
    let id = scene.world.ids_with_mesh()[0];
    let start = frame(&mut r, &scene).0;
    r.draw_poses.record(&scene);
    scene.world.transform_mut(id).unwrap().position += Vec3::new(1.5, 0.0, 0.0);
    r.draw_poses.record(&scene);
    r.draw_poses.set_alpha(0.0);
    assert_eq!(frame(&mut r, &scene).0, start, "alpha 0: the tick before");
    r.draw_poses.set_alpha(0.5);
    let half = frame(&mut r, &scene).0;
    r.draw_poses.set_alpha(1.0);
    let at_tick = frame(&mut r, &scene).0;
    assert_ne!(half, start);
    assert_ne!(half, at_tick);
    drop(r);
    let Some(mut fresh) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    assert_eq!(
        frame(&mut fresh, &scene).0,
        at_tick,
        "alpha 1: the live scene"
    );
}
