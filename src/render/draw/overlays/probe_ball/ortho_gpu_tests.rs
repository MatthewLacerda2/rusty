//! The probe ball under an orthographic camera (#1911's review): every ray is
//! parallel, so the ball's quad faces the view direction, not the eye, and a probe
//! off to the side keeps its whole silhouette.

use glam::Vec3;

use super::gpu_tests::{assert_near, faced_cube, frame, pixel, srgb, FACES, RES};
use crate::components::rendering::camera::Projection;
use crate::core::application::SceneGizmos;
use crate::scene::{Camera, Scene};

/// A probe 45° off the eye's axis: half a metre right of and in front of an
/// orthographic camera at (0, 1, 2) looking down -Z, one metre half-height. At 32
/// pixels a metre the ball (0.25 m radius) is 8 pixels across each way of column 48.
#[test]
fn gpu_an_orthographic_camera_draws_an_off_axis_ball_whole() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    let i = scene
        .reflection_probes
        .add(Vec3::new(0.5, 1.0, 1.5), Vec3::splat(2.0));
    let cube = faced_cube("probe_ball_ortho_faces.ktx2");
    scene.reflection_probes.set_cubemap(i, cube);
    let mut cam = Camera::new(Vec3::new(0.0, 1.0, 2.0), -90.0, 0.0);
    cam.projection = Projection::Orthographic { size: 1.0 };
    let px = frame(&mut r, &scene, &cam, SceneGizmos::default());
    let row = RES / 2;
    // The centre: the view ray (-Z) meets the normal +Z and reflects along +Z.
    assert_near(
        pixel(&px, 48, row),
        srgb(FACES[4]),
        "the centre, the +Z face",
    );
    // 6.5 px out, past the 6 px a quad facing the eye would leave: the normal leans
    // +X enough that the reflection is mostly +X.
    assert_near(pixel(&px, 54, row), srgb(FACES[0]), "the edge, the +X face");
}
