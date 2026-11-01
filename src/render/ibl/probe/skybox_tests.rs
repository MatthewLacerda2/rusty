//! A light probe in `Skybox` mode (#1675): an empty scene's GPU-baked probe takes
//! the panorama's light as the shader shades an unbaked surface with it
//! (`AmbientRadiance::at`, which `ambient_gpu_tests` holds the shader to and the
//! lightmap's `skybox_tests` the trace), and a different panorama moves it.

use glam::Vec3;

use crate::scene::{panorama_fixture, AmbientMode, Scene};

const RES: u32 = 32;

#[test]
fn gpu_an_empty_panorama_scene_bakes_the_skys_light() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut ups = Vec::new();
    for (name, top, bottom) in [
        ("a.png", [60, 100, 220], [160, 110, 60]),
        ("b.png", [160, 110, 60], [60, 100, 220]),
    ] {
        let mut scene = Scene::new();
        scene.ambient.mode = AmbientMode::Skybox;
        scene.ambient_intensity = 1.5;
        scene.skybox_path = panorama_fixture(name, top, bottom, 40);
        let want = scene.drawn_ambient().radiance();
        let baked = renderer.bake_probe(&scene, Vec3::ZERO, RES);
        for n in [
            Vec3::Y,
            Vec3::NEG_Y,
            Vec3::X,
            Vec3::NEG_X,
            Vec3::new(0.0, 0.6, 0.8),
        ] {
            let got = baked.eval(n) / std::f32::consts::PI;
            assert!(
                got.abs_diff_eq(want.at(n), 5e-3),
                "{name} {n}: {got} vs {}",
                want.at(n)
            );
        }
        ups.push(baked.eval(Vec3::Y));
    }
    assert!(ups[0].z > ups[0].x && ups[1].x > ups[1].z, "{ups:?}");
}
