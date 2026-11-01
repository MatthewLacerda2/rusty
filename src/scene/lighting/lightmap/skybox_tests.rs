//! The lightmap trace in `Skybox` mode (#1675): an unoccluded texel bakes to the
//! sky's light as the shader shades an unbaked surface with it, the panorama's and
//! the procedural sky's, and a different panorama moves it.

use glam::Vec3;

use super::ambient_tests::open_texel;
use crate::scene::{panorama_fixture, AmbientMode, Scene};

#[test]
fn an_open_texel_bakes_the_skys_light() {
    let mut scenes = Vec::new();
    for (name, top, bottom) in [
        ("a.png", [60, 100, 220], [160, 110, 60]),
        ("b.png", [160, 110, 60], [60, 100, 220]),
        ("", [0; 3], [0; 3]),
    ] {
        let mut scene = Scene::new();
        scene.ambient.mode = AmbientMode::Skybox;
        if !name.is_empty() {
            scene.skybox_path = panorama_fixture(name, top, bottom, 40);
        }
        scenes.push(scene);
    }
    let mut ups = Vec::new();
    for scene in &scenes {
        let sky = scene.drawn_ambient().radiance();
        for n in [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::new(0.0, -0.6, 0.8)] {
            // Monte Carlo noise only, as the Color and Gradient cases.
            let (baked, want) = (open_texel(sky, n), sky.at(n));
            assert!(
                (baked - want).abs().max_element() < 0.02,
                "{n}: {baked} vs {want}"
            );
        }
        ups.push(open_texel(sky, Vec3::Y));
    }
    assert!(ups[0].z > ups[0].x && ups[1].x > ups[1].z, "{ups:?}");
}
