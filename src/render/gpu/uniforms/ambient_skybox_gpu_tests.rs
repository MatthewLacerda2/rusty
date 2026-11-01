//! `Skybox` mode's ambient on the GPU (#1675): the shader evaluates the sky's SH the
//! CPU's `AmbientRadiance::at` does, for a panorama and for the procedural sky, and
//! a different panorama moves it.

use super::ambient_gpu_tests::{cpu_gap, probe};
use crate::scene::{AmbientMode, Scene};

#[test]
fn gpu_the_skybox_ambient_is_the_cpus() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    // The panorama's SH, and the procedural sky's, as the CPU reads
    // them; a different panorama moves the GPU's too.
    let mut skies = Vec::new();
    for (name, top, bottom) in [
        ("a.png", [60, 100, 220], [160, 110, 60]),
        ("b.png", [160, 110, 60], [60, 100, 220]),
    ] {
        let mut sky = Scene::new();
        sky.ambient.mode = AmbientMode::Skybox;
        sky.skybox_path = crate::scene::panorama_fixture(name, top, bottom, 40);
        skies.push(sky);
    }
    let mut procedural = Scene::new();
    procedural.ambient.mode = AmbientMode::Skybox;
    procedural.skybox_path = String::new();
    let mut ups = Vec::new();
    for scene in skies.iter().chain([&procedural]) {
        let gpu = probe(&r, scene);
        let gap = cpu_gap(scene, &gpu);
        assert!(gap <= 1e-5, "Skybox: CPU and GPU disagree by {gap}");
        ups.push(gpu[0].0);
    }
    assert!(ups[0].z > ups[0].x && ups[1].x > ups[1].z, "{ups:?}");
}
