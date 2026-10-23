//! Runtime shader params per material slot (#1285): one entity's mesh is two runs
//! whose materials share a hit-flash shader, the left flashing red and the right
//! blue. With an entity override set (#670) each slot packs its own material's
//! values into a buffer of its own, so each half keeps its colour; one shared
//! per-entity buffer would paint both halves with whichever slot wrote last.
//! Skips with no adapter.

use std::cell::RefCell;

use glam::Vec3;

use super::surface::params_tests::{halves, lua, Flash, RES};
use crate::components::mesh::Vertex;
use crate::components::{MaterialAsset, MaterialComponent, MeshComponent};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A 4 x 2 quad facing +Z, split at x = 0 into two runs: -X, then +X.
fn halves_quad() -> MeshComponent {
    let (mut vertices, mut indices, mut ranges) = (Vec::new(), Vec::new(), Vec::new());
    for (x0, x1) in [(-2.0, 0.0), (0.0, 2.0)] {
        let base = vertices.len() as u32;
        for (x, y) in [(x0, -1.0), (x1, -1.0), (x1, 1.0), (x0, 1.0)] {
            vertices.push(Vertex::new(Vec3::new(x, y, 0.0), Vec3::Z, [0.0, 0.0]));
        }
        let start = indices.len() as u32;
        indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        ranges.push(start..indices.len() as u32);
    }
    MeshComponent {
        ranges,
        ..MeshComponent::new("FlashHalves", vertices, indices)
    }
}

#[test]
fn gpu_each_slot_keeps_its_own_shader_params_under_an_entity_override() {
    let flash = Flash::bake_as("_slots");
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let id = scene.add_entity("Quad".into());
    scene.world.set_mesh(id, Some(halves_quad()));
    for key in ["red_flash", "blue_flash"] {
        let asset = MaterialAsset {
            shader: Some(flash.0.clone()),
            ..MaterialAsset::default()
        };
        scene.materials.insert(key.into(), asset);
    }
    let materials = vec!["red_flash".into(), "blue_flash".into()];
    scene
        .world
        .set_material(id, Some(MaterialComponent { materials }));
    let scene = RefCell::new(scene);
    lua(
        &scene,
        &format!(
            r#"Material.SetAssetShaderParam("red_flash", "hit_flash.color", {{1, 0, 0}})
               Material.SetAssetShaderParam("blue_flash", "hit_flash.color", {{0, 0, 1}})
               Material.SetShaderParam({id}, "hit_flash.amount", 1)"#
        ),
    );
    // Twice: a buffer the two slots shared would be rewritten between their draws.
    halves(&mut renderer, &scene.borrow());
    let [(lr, lb), (rr, rb)] = halves(&mut renderer, &scene.borrow());
    eprintln!("[slot params] left (r {lr}, b {lb}), right (r {rr}, b {rb})");
    assert!(lr > lb * 2, "the left half flashes red: r {lr}, b {lb}");
    assert!(rb > rr * 2, "the right half flashes blue: r {rr}, b {rb}");
}
