//! Phase two of GPU occlusion culling (#1963): a box that comes out from behind
//! the wall is drawn the frame it does, though last frame's set left it out.

use super::gpu_tests::{drawn, facing_the_wall, offscreen, shot, walled, H, W};

#[test]
fn gpu_a_box_out_from_behind_the_wall_is_drawn_the_same_frame() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (mut scene, boxed) = walled();
    let cam = facing_the_wall();
    let mut view = offscreen(&r);
    for _ in 0..2 {
        shot(&mut r, &mut view, &scene, &cam);
    }
    assert_eq!(
        drawn(&r, &view).iter().sum::<u32>(),
        1,
        "hidden, two frames"
    );
    // Out in front of the wall, in sight, and low enough that its top shows (its
    // front is shaded as the wall is): phase one skips it, phase two draws it.
    {
        let mut t = scene.world.transform_mut(boxed).unwrap();
        t.position += glam::Vec3::new(0.0, -1.0, 8.0);
    }
    let (on, _) = shot(&mut r, &mut view, &scene, &cam);
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 2, "drawn this frame");
    r.quality.gpu_occlusion_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(on == off, "the box came out a frame late");
    // And it shows: without it the frame differs.
    scene.world.transform_mut(boxed).unwrap().position.y -= 100.0;
    let mut fresh = offscreen(&r);
    let (gone, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(
        gone != on,
        "the box is not in view, so the test proves nothing"
    );
}
