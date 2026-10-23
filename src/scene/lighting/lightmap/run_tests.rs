//! A multi-material mesh bakes each run with its own material (#1290), paired with
//! its slots the way the renderer draws them.

use glam::{Vec2, Vec3};

use super::tests::{floor, settings};
use super::*;
use crate::components::mesh::Vertex;
use crate::components::{MaterialAsset, MaterialComponent, MeshComponent, RenderMode};
use crate::scene::Scene;

/// A static 4×4 panel at y = 1 facing down, as two runs: the half over x < 0, then
/// the half over x > 0. No lightmap UV: it only occludes, bounces and emits.
fn panel(scene: &mut Scene, slots: Option<&[&str]>) -> u32 {
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
/// (Transparent) material in its library.
fn library() -> Scene {
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
        ..glow.clone()
    };
    for (key, m) in [("black", black), ("glow", glow), ("glass", glass)] {
        scene.materials.insert(key.to_string(), m);
    }
    scene
}

/// The panel's gathered index count and each run's (end, albedo.x, emissive.x).
fn gathered(slots: Option<&[&str]>) -> (usize, Vec<(u32, f32, f32)>) {
    let mut scene = library();
    panel(&mut scene, slots);
    let baked = BakeScene::gather(&scene, &|_| None);
    let Some(mesh) = baked.meshes.first() else {
        return (0, Vec::new());
    };
    let runs = mesh.runs.iter().map(|r| (r.end, r.albedo.x, r.emissive.x));
    (mesh.indices.len(), runs.collect())
}

#[test]
fn each_run_bakes_with_its_own_slot() {
    assert_eq!(
        gathered(Some(&["black", "glow"])),
        (12, vec![(6, 0.0, 0.0), (12, 0.0, 1.0)])
    );
    assert_eq!(
        gathered(None),
        (12, vec![(6, 1.0, 0.0), (12, 1.0, 0.0)]),
        "no Material: every run is the default white"
    );
    assert_eq!(
        gathered(Some(&["glow"])),
        (6, vec![(6, 0.0, 1.0)]),
        "a run with no slot is left out"
    );
    assert_eq!(
        gathered(Some(&["black", "black", "glow"])),
        (12, vec![(6, 0.0, 0.0), (12, 0.0, 1.0)]),
        "an extra slot adds its emission to the last run"
    );
}

#[test]
fn a_transparent_run_is_left_out_not_the_whole_mesh() {
    assert_eq!(gathered(Some(&["glass", "glow"])), (6, vec![(6, 0.0, 1.0)]));
    assert_eq!(
        gathered(Some(&["black", "glass"])),
        (6, vec![(6, 0.0, 0.0)])
    );
    assert_eq!(
        gathered(Some(&["black", "glass", "glow"])),
        (6, vec![(6, 0.0, 0.0)]),
        "an extra slot over a left-out run adds nothing"
    );
    assert_eq!(gathered(Some(&["glass", "glass"])), (0, Vec::new()));
}

/// Mean brightness of the floor's lightmap columns under x < 0, then under x > 0.
fn floor_halves(slots: &[&str]) -> (f32, f32) {
    let mut scene = library();
    scene.ambient_intensity = 0.0;
    panel(&mut scene, Some(slots));
    let mut baked = BakeScene::gather(&scene, &|_| None);
    let receiver = BakeMesh {
        entity: 999,
        ..floor()
    };
    assert!(receiver.lightmap_uvs.iter().any(|uv| *uv != Vec2::ZERO));
    baked.meshes.push(receiver);
    let maps = bake(&baked, &settings());
    let map = maps.iter().find(|m| m.entity == 999).expect("floor baked");
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

#[test]
fn only_the_emissive_half_lights_the_floor_below_it() {
    let (left, right) = floor_halves(&["black", "glow"]);
    assert!(right > 0.05 && right > 2.0 * left, "{left} | {right}");
    let (left, right) = floor_halves(&["glow", "black"]);
    assert!(left > 0.05 && left > 2.0 * right, "{left} | {right}");
}
