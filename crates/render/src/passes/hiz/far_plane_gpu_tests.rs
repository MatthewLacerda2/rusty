//! A wall in the last percent of the view: reversed depth cancels near the far plane, so the
//! box test's corner depth and the face the wall rasterized round apart by more there.

use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_a_wall_near_the_far_plane_stays_drawn_frame_after_frame() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let cases = [
        (200.0, 0.98),
        (1000.0, 0.97),
        (1000.0, 0.99),
        (1000.0, 0.999),
        (5.0e4, 0.99),
    ];
    for eye in [Vec3::ZERO, Vec3::new(1234.567, 31.3, 2345.678)] {
        for (far, at) in cases {
            let d: f32 = far * at;
            let mut scene = Scene::new();
            scene.skybox_path = String::new();
            let (lo, hi) = (Vec3::new(-d, -d, -d - 1.0), Vec3::new(d, d, -d));
            block(&mut scene, "Wall", eye + lo, eye + hi);
            let lo = eye + Vec3::new(-0.5, -0.5, -3.0);
            block(&mut scene, "Box", lo, lo + Vec3::ONE);
            let mut cam = Camera::new(eye, -90.0, 0.0);
            cam.far = far;
            let mut view = offscreen(&r);
            for frame in 0..4 {
                shot(&mut r, &mut view, &scene, &cam);
                let n = drawn(&r, &view).iter().sum::<u32>();
                assert_eq!(
                    n, 2,
                    "far {far}, the wall at {d} m, frame {frame}: {n} drawn"
                );
            }
        }
    }
}

#[test]
fn gpu_a_wall_near_the_far_plane_still_hides_what_is_behind_it() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let eye = Vec3::new(1234.567, 31.3, 2345.678);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (
        Vec3::new(-900.0, -900.0, -901.0),
        Vec3::new(900.0, 900.0, -900.0),
    );
    block(&mut scene, "Wall", eye + lo, eye + hi);
    let lo = eye + Vec3::new(-5.0, -5.0, -960.0);
    block(&mut scene, "Box", lo, lo + Vec3::splat(10.0));
    let mut cam = Camera::new(eye, -90.0, 0.0);
    cam.far = 1000.0;
    let mut view = offscreen(&r);
    let mut culled = 0;
    for _ in 0..3 {
        let (_, c) = shot(&mut r, &mut view, &scene, &cam);
        culled = culled.max(c.hiz_culled);
    }
    assert_eq!(
        culled, 1,
        "the box 60 m behind the wall, 96% of the way out"
    );
    assert_eq!(drawn(&r, &view).iter().sum::<u32>(), 1, "the wall alone");
}
