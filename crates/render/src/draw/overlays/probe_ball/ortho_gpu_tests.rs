use glam::Vec3;

use super::gpu_tests::{assert_near, faced_cube, frame, pixel, srgb, FACES, RES};
use rusty_components::rendering::camera::Projection;
use rusty_core::application::SceneGizmos;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_an_orthographic_camera_draws_an_off_axis_ball_whole() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
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
    assert_near(
        pixel(&px, 48, row),
        srgb(FACES[4]),
        "the centre, the +Z face",
    );
    // Column 54 is 6.5 px out, past the 6 px a quad facing the eye would leave.
    assert_near(pixel(&px, 54, row), srgb(FACES[0]), "the edge, the +X face");
}
