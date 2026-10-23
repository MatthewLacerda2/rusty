//! A multi-material mesh bakes each run with its own material (#1290), paired with
//! its slots the way the renderer draws them.

use std::cell::Cell;

use glam::Vec3;

use super::*;
use crate::asset::MapRef;
use crate::components::mesh::Vertex;
use crate::components::{MaterialAsset, MaterialComponent, MeshComponent, RenderMode};
use crate::scene::Scene;

/// A static 4×4 panel at y = 1 facing down, as two runs: the half over x < 0, then
/// the half over x > 0. No lightmap UV: it only occludes, bounces and emits.
pub(super) fn panel(scene: &mut Scene, slots: Option<&[&str]>) -> u32 {
    let half = |x0: f32, x1: f32| {
        let p = |x, z| Vertex::new(Vec3::new(x, 1.0, z), Vec3::NEG_Y, [0.0; 2]);
        [p(x0, -2.0), p(x1, -2.0), p(x1, 2.0), p(x0, 2.0)]
    };
    let vertices = [half(-2.0, 0.0), half(0.0, 2.0)].concat();
    let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let mesh = MeshComponent {
        ranges: vec![0..6, 6..12],
        ..MeshComponent::new("Panel", vertices, indices)
    };
    let id = scene.add_entity("Panel".to_string());
    scene.world.set_mesh(id, Some(mesh));
    scene.world.set_static(id, true);
    let materials = slots.map(|s| MaterialComponent {
        materials: s.iter().map(|k| k.to_string()).collect(),
    });
    scene.world.set_material(id, materials);
    id
}

/// A scene with a `black`, a `glow` (black, emitting white) and a `glass`
/// (Transparent at alpha 0.5, glowing) material in its library.
pub(super) fn library() -> Scene {
    let mut scene = Scene::new();
    let black = MaterialAsset {
        base_color: [0.0; 3],
        ..MaterialAsset::default()
    };
    let glow = MaterialAsset {
        emissive: [1.0; 3],
        ..black.clone()
    };
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..glow.clone()
    };
    for (key, m) in [("black", black), ("glow", glow), ("glass", glass)] {
        scene.materials.insert(key.to_string(), m);
    }
    scene
}

/// The panel's gathered index count and each run's (end, albedo.x, emissive.x,
/// occludes).
fn gathered(slots: Option<&[&str]>) -> (usize, Vec<(u32, f32, f32, bool)>) {
    let mut scene = library();
    panel(&mut scene, slots);
    let baked = BakeScene::gather(&scene, &|_| None);
    let Some(mesh) = baked.meshes.first() else {
        return (0, Vec::new());
    };
    let runs = mesh
        .runs
        .iter()
        .map(|r| (r.end, r.albedo.x, r.emissive.x, r.occludes));
    (mesh.indices.len(), runs.collect())
}

#[test]
fn each_run_bakes_with_its_own_slot() {
    let (black, white) = ((0.0, 0.0, true), (1.0, 0.0, true));
    let at = |end, (a, e, o): (f32, f32, bool)| (end, a, e, o);
    let glow = (0.0, 1.0, true);
    assert_eq!(
        gathered(Some(&["black", "glow"])),
        (12, vec![at(6, black), at(12, glow)])
    );
    assert_eq!(
        gathered(None),
        (12, vec![at(6, white), at(12, white)]),
        "no Material: every run is the default white"
    );
    assert_eq!(
        gathered(Some(&["glow"])),
        (6, vec![at(6, glow)]),
        "a run with no slot is left out"
    );
    assert_eq!(
        gathered(Some(&["black", "black", "glow"])),
        (12, vec![at(6, black), at(12, glow)]),
        "an extra slot adds its emission to the last run"
    );
}

/// A Transparent run stays drawn (it receives its lightmap) but lets light through;
/// an extra slot over it is judged by its own material (#1290).
#[test]
fn a_transparent_run_receives_but_does_not_occlude() {
    let glass = |end| (end, 0.0, 0.5, false);
    assert_eq!(
        gathered(Some(&["glass", "glow"])),
        (12, vec![glass(6), (12, 0.0, 1.0, true)])
    );
    assert_eq!(
        gathered(Some(&["glass", "glass"])),
        (12, vec![glass(6), glass(12)])
    );
    assert_eq!(
        gathered(Some(&["black", "glass", "glow"])),
        (12, vec![(6, 0.0, 0.0, true), (12, 0.0, 1.5, true)]),
        "an opaque layer over glass occludes"
    );
    assert_eq!(
        gathered(Some(&["black", "black", "glass"])),
        (12, vec![(6, 0.0, 0.0, true), (12, 0.0, 0.5, true)]),
        "a Transparent layer adds its emission scaled by alpha"
    );
}

/// A texture several slots share is averaged once per gather.
#[test]
fn a_shared_map_is_averaged_once() {
    let mut scene = library();
    let mapped = MaterialAsset {
        base_color_map: Some(MapRef::from("t.png")),
        ..MaterialAsset::default()
    };
    scene.materials.insert("mapped".to_string(), mapped);
    panel(&mut scene, Some(&["mapped", "mapped", "mapped"]));
    let asked = Cell::new(0);
    let baked = BakeScene::gather(&scene, &|_| {
        asked.set(asked.get() + 1);
        Some(Vec3::splat(0.5))
    });
    assert_eq!(asked.get(), 1);
    assert!(baked.meshes[0]
        .runs
        .iter()
        .all(|r| r.albedo == Vec3::splat(0.5)));
}
