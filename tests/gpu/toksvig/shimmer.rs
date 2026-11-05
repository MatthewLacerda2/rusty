//! A far busy glossy metal floor, the camera turning: the busy map shimmers far
//! less than its twin (`main`'s look), keeps its light, and is the twin's frame up
//! close. The BC5 route (the importer's per-level table) draws what the
//! uncompressed one (the length in each sample) draws.

use glam::Vec3;
use rusty::components::LightComponent;
use rusty::scene::Scene;

use super::{as_bc5, floor, frames, glossy, luma, mean, normal_map, Blue, FAR, NEAR};
use crate::gpu::specular_aa_screenshot::relative_temporal_variance;

const FRAMES: u32 = 8;
/// Far, relative temporal variance: the twin's sparkle, the busy map's at most.
const TWIN_SHIMMERS: f64 = 0.4;
const BOUND: f64 = 0.2;

/// The floor, metal of `roughness` with `map`, under one lamp of `intensity`.
fn scene(map: &std::path::Path, roughness: f32, intensity: f32) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    floor(&mut scene, "Floor", 0.0, glossy(map, 1.0, roughness));
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 1.2, -4.0);
    let light = LightComponent {
        intensity,
        range: 30.0,
        cast_shadows: false,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

/// What one map draws: far variance and mean (glossy), the near frames' luma,
/// and the far mean at roughness 0.3 under a dim lamp, where nothing clips.
struct Drawn {
    variance: f64,
    far: f64,
    near: Vec<Vec<f64>>,
    dim: f64,
}

fn draw(map: &std::path::Path) -> Option<Drawn> {
    let glossy = scene(map, 0.08, 30.0);
    let far = luma(&frames(&glossy, FAR, FRAMES)?);
    let near = luma(&frames(&glossy, NEAR, FRAMES)?);
    let dim = mean(&luma(&frames(&scene(map, 0.3, 1.0), FAR, FRAMES)?));
    let (variance, _) = relative_temporal_variance(&far);
    let far = mean(&far);
    eprintln!(
        "[toksvig] {}: far variance {variance:.3}, means far {far:.2} near {:.2} dim {dim:.2}",
        map.display(),
        mean(&near)
    );
    Some(Drawn {
        variance,
        far,
        near,
        dim,
    })
}

#[test]
fn a_far_busy_glossy_floor_shimmers_less_keeps_its_light_and_is_unchanged_up_close() {
    let Some(twin) = draw(&normal_map("rusty_1702_twin.png", true, Blue::Pinned(255))) else {
        return;
    };
    let rgba = draw(&normal_map("rusty_1702_rgba.png", true, Blue::Z)).unwrap();
    let bc5_map = normal_map("rusty_1702_bc5.png", true, Blue::Z);
    as_bc5(&bc5_map);
    let bc5 = draw(&bc5_map).unwrap();
    assert!(
        twin.variance > TWIN_SHIMMERS,
        "the twin sparkles: {:.3}",
        twin.variance
    );
    assert!(rgba.near == twin.near, "the base level widens nothing");
    for (name, d) in [("RGBA8", &rgba), ("BC5", &bc5)] {
        assert!(d.variance < BOUND, "{name} shimmers: {:.3}", d.variance);
        // The twin's sparkles clip at 255, so its far mean understates its light.
        assert!(d.far > twin.far, "{name} lost its far light: {:.2}", d.far);
        let ratio = d.dim / twin.dim;
        assert!(
            (0.9..1.15).contains(&ratio),
            "{name} moved the energy: {ratio:.3}"
        );
    }
    // The two routes agree, far and near (BC5's own rounding aside).
    let far = bc5.far / rgba.far;
    assert!(
        (0.85..1.15).contains(&far),
        "BC5's far {:.2} vs RGBA8's {:.2}",
        bc5.far,
        rgba.far
    );
    let near = (mean(&bc5.near) - mean(&rgba.near)).abs();
    assert!(near < 1.0, "BC5 changed up close by {near:.2}");
}
