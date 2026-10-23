//! What each material run does to the bake (#1290): an emissive run lights only what
//! is under it, and a Transparent run receives its share of the entity's lightmap
//! but neither shadows nor bounces, since the renderer samples the lightmap for
//! glass too.

use glam::Vec3;

use super::run_tests::{library, panel};
use super::tests::{floor, settings};
use super::*;
use crate::components::mesh::Vertex;
use crate::components::{MaterialComponent, MeshComponent};

/// The mean brightness of `map`'s columns left of its middle, then right of it.
fn halves(map: &Lightmap) -> (f32, f32) {
    let size = map.size as usize;
    let half = |right: bool| {
        let texels = map.texels.iter().enumerate();
        let picked: Vec<f32> = texels
            .filter(|(i, _)| (i % size >= size / 2) == right)
            .map(|(_, t)| t.x)
            .collect();
        picked.iter().sum::<f32>() / picked.len() as f32
    };
    (half(false), half(true))
}

/// Mean brightness of the floor's lightmap columns under a downward panel's x < 0
/// half, then under its x > 0 half.
fn floor_halves(slots: &[&str]) -> (f32, f32) {
    let mut scene = library();
    scene.ambient_intensity = 0.0;
    panel(&mut scene, Some(slots));
    let mut baked = BakeScene::gather(&scene, &|_| None);
    baked.meshes.push(BakeMesh {
        entity: 999,
        ..floor()
    });
    let maps = bake(&baked, &settings());
    halves(maps.iter().find(|m| m.entity == 999).expect("floor baked"))
}

#[test]
fn only_the_emissive_half_lights_the_floor_below_it() {
    let (left, right) = floor_halves(&["black", "glow"]);
    assert!(right > 0.05 && right > 2.0 * left, "{left} | {right}");
    let (left, right) = floor_halves(&["glow", "black"]);
    assert!(left > 0.05 && left > 2.0 * right, "{left} | {right}");
}

#[test]
fn glass_is_lit_and_lets_light_through_to_the_floor() {
    let mut scene = library();
    scene.ambient_intensity = 0.0;
    // A 4×4 pane at y = 1 facing up: a black frame run over x < 0, a glass run over
    // x > 0, each charted on its half of the lightmap.
    let half = |x0: f32, x1: f32| {
        let p = |x: f32, z: f32| Vertex {
            lightmap_uv: [(x + 2.0) / 4.0, (z + 2.0) / 4.0],
            ..Vertex::new(Vec3::new(x, 1.0, z), Vec3::Y, [0.0; 2])
        };
        [p(x0, 2.0), p(x1, 2.0), p(x1, -2.0), p(x0, -2.0)]
    };
    let mesh = MeshComponent {
        ranges: vec![0..6, 6..12],
        ..MeshComponent::new(
            "Pane",
            [half(-2.0, 0.0), half(0.0, 2.0)].concat(),
            vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        )
    };
    let pane = scene.add_entity("Pane".to_string());
    scene.world.set_mesh(pane, Some(mesh));
    scene.world.set_static(pane, true);
    let materials = ["black", "glass"].map(String::from).to_vec();
    scene
        .world
        .set_material(pane, Some(MaterialComponent { materials }));

    let mut baked = BakeScene::gather(&scene, &|_| None);
    baked.meshes.push(BakeMesh {
        entity: 999,
        ..floor()
    });
    baked.lights.push(BakeLight {
        shape: LightShape::Directional {
            direction: Vec3::NEG_Y,
        },
        radiance: Vec3::splat(3.0),
        bakes_direct: true,
    });
    let maps = bake(&baked, &settings());
    let map = |entity| maps.iter().find(|m| m.entity == entity).expect("baked");

    let (_, glass) = halves(map(pane));
    assert!(glass > 0.1, "the glass run's chart is lit: {glass}");
    let (shadowed, through) = halves(map(999));
    assert!(
        through > 0.1 && through > 2.0 * shadowed,
        "{shadowed} | {through}"
    );
}
