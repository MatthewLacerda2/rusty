//! GPU occlusion culling at its edges (#1963): an empty scene and a lone box.

use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use crate::scene::lighting::occlusion::test_level::block;
use crate::scene::{Camera, Scene};

#[test]
fn gpu_an_empty_scene_and_a_lone_box_cull_nothing() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
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
