//! The lightmap bake, continued (#438): the sky level, meshes without a lightmap UV,
//! texel density, and gathering a live [`Scene`]'s static geometry and lights.

use glam::{Vec2, Vec3};

use super::tests::{floor, mean, settings};
use super::*;
use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::Scene;

#[test]
fn an_open_floor_bakes_to_the_sky_gradient() {
    let scene = BakeScene {
        meshes: vec![floor()],
        lights: vec![],
        sky: Vec3::ONE,
    };
    let s = BakeSettings {
        samples: 256,
        ..settings()
    };
    // Cosine-weighted mean of the sky gradient 0.625 + 0.375·y over the upper
    // hemisphere, where y averages 2/3: 0.875.
    let value = mean(&bake(&scene, &s), 1);
    assert!((value - 0.875).abs() < 0.03, "{value}");
}

#[test]
fn a_mesh_without_lightmap_uvs_gets_no_lightmap_but_still_occludes() {
    let mut roof = floor();
    roof.entity = 3;
    roof.lightmap_uvs = vec![Vec2::ZERO; 4];
    // Raise it and flip it to face down onto the floor.
    roof.positions = roof.positions.iter().map(|p| *p + Vec3::Y).rev().collect();
    roof.normals = vec![Vec3::NEG_Y; 4];
    assert_eq!(lightmap_size(&roof, 4.0, 64), None);
    let scene = BakeScene {
        meshes: vec![floor(), roof],
        lights: vec![],
        sky: Vec3::ONE,
    };
    let maps = bake(&scene, &settings());
    assert_eq!(maps.len(), 1, "only the floor is lightmapped");
    assert!(
        mean(&maps, 1) < 0.875 * 0.7,
        "the roof shades the floor from the sky"
    );
}

#[test]
fn lightmap_size_follows_texel_density() {
    let f = floor(); // 16 m² over the whole [0,1]² UV square: 4 m per UV unit.
    assert_eq!(lightmap_size(&f, 4.0, 512), Some(16));
    assert_eq!(lightmap_size(&f, 4.0, 8), Some(8), "clamped to the max");
    assert_eq!(lightmap_size(&f, 0.1, 512), Some(MIN_RESOLUTION));
}

#[test]
fn gather_takes_static_meshes_and_baked_in_lights_only() {
    let mut scene = Scene::new();
    let box_entity = |scene: &mut Scene, name: &str, is_static| {
        let id = scene.add_entity(name.to_string());
        scene
            .world
            .set_mesh(id, primitive_mesh_component(Primitive::Box));
        scene.world.set_static(id, is_static);
        id
    };
    let wall = box_entity(&mut scene, "Wall", true);
    box_entity(&mut scene, "Crate", false);
    for mode in [LightMode::Realtime, LightMode::Mixed, LightMode::Baked] {
        let id = scene.add_entity(mode.name().to_string());
        let light = LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 2.0,
            range: 10.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            mode,
        };
        scene.world.set_light(id, Some(light));
    }
    let gathered = BakeScene::gather(&scene, &|_| None);
    let entities: Vec<u32> = gathered.meshes.iter().map(|m| m.entity).collect();
    assert_eq!(entities, vec![wall], "only the static mesh is baked");
    let direct: Vec<bool> = gathered.lights.iter().map(|l| l.bakes_direct).collect();
    assert_eq!(
        direct,
        vec![false, true],
        "Realtime is left out; Mixed, then Baked"
    );
    assert!(
        lightmap_size(&gathered.meshes[0], 4.0, 64).is_some(),
        "a Box has UV2"
    );
}
