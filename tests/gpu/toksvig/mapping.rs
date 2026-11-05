//! The level a world mapping reads the normal map at (`mapping::normal_level`):
//! under Planar and Triplanar a far busy floor shimmers less than its twin, as
//! under UV0 (`shimmer`). And a map whose blue is not the normal's z (an RG export's
//! 128) reads as whole: it draws what a flat map with blue 255 draws, far off.

use rusty::components::{MaterialAsset, UvMapping};
use rusty::scene::Scene;

use super::{floor, frames, glossy, luma, normal_map, Blue, FAR};
use crate::gpu::specular_aa_screenshot::relative_temporal_variance;

/// A lit glossy metal floor of `material`, a lamp at its far end, black sky.
fn scene(material: MaterialAsset) -> Scene {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    floor(&mut scene, "Floor", 0.0, material);
    let lamp = scene.add_entity("Lamp".into());
    scene.world.transform_mut(lamp).unwrap().position = glam::Vec3::new(0.0, 1.2, -4.0);
    let light = rusty::components::LightComponent {
        intensity: 30.0,
        range: 30.0,
        cast_shadows: false,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene
}

/// The far relative temporal variance of `map` read under `mapping`.
fn variance(map: &std::path::Path, mapping: UvMapping) -> Option<f64> {
    let material = MaterialAsset {
        uv_mapping: mapping,
        world_size: 15.0,
        ..glossy(map, 1.0, 0.08)
    };
    Some(relative_temporal_variance(&luma(&frames(&scene(material), FAR, 8)?)).0)
}

#[test]
fn a_world_mapped_busy_floor_shimmers_less_than_its_twin() {
    let busy = normal_map("rusty_1702_world_busy.png", true, Blue::Z);
    let twin = normal_map("rusty_1702_world_twin.png", true, Blue::Pinned(255));
    for mapping in [UvMapping::Planar, UvMapping::Triplanar] {
        let Some(widened) = variance(&busy, mapping) else {
            return;
        };
        let plain = variance(&twin, mapping).unwrap();
        eprintln!("[toksvig] {mapping:?}: far variance {widened:.3} vs the twin's {plain:.3}");
        assert!(
            plain > 0.4 && widened < 0.2,
            "{mapping:?}: {widened:.3} vs {plain:.3}"
        );
    }
}

#[test]
fn a_flat_map_with_blue_128_draws_what_one_with_blue_255_draws() {
    let rg = normal_map("rusty_1702_flat_128.png", false, Blue::Pinned(128));
    let whole = normal_map("rusty_1702_flat_255.png", false, Blue::Pinned(255));
    let Some(a) = frames(&scene(glossy(&rg, 1.0, 0.08)), FAR, 2) else {
        return;
    };
    let b = frames(&scene(glossy(&whole, 1.0, 0.08)), FAR, 2).unwrap();
    assert!(a == b, "a blue of 128 is not a spread of normals");
}
