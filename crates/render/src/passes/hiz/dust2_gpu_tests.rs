//! GPU occlusion culling on the Dust2-sized bench level (#1963), measured by hand:
//! `cargo nextest run --features dev --run-ignored only -E 'test(hiz_dust2)'
//! --no-capture`. Each eye's drawn instances on and off, and the same pixels;
//! then, with each room's statue an 8064-triangle clustered sphere (#1968), the
//! triangles drawn with cluster culling on and off.

use std::sync::Arc;

use glam::Vec3;

use super::cluster_gpu_tests::cluster_indices;
use super::gpu_tests::{drawn, offscreen, shot, words, H, W};
use crate::{RenderView, Renderer};
use rusty_components::mesh::{primitives, MeshRevision};
use rusty_scene::lighting::occlusion::bench::level::rooms;
use rusty_scene::Camera;
use rusty_scene::Scene;

#[test]
// lint: allow(skip) a measurement on the 6344-mesh level, run by hand, never a gate
#[ignore = "a measurement, run by hand"]
fn gpu_hiz_dust2_counts() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = rooms(20);
    for (name, at, yaw, pitch) in EYES {
        let cam = Camera::new(at, yaw, pitch);
        let mut view = offscreen(&r);
        r.quality.gpu_occlusion_culling = true;
        shot(&mut r, &mut view, &scene, &cam);
        let (on, c) = shot(&mut r, &mut view, &scene, &cam);
        let kept: u32 = drawn(&r, &view).iter().sum();
        r.quality.gpu_occlusion_culling = false;
        let mut fresh = offscreen(&r);
        let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
        let same = ["PIXELS DIFFER", "same pixels"][usize::from(on == off)];
        println!(
            "{name}: {} meshes drawn by the CPU's culls, {} instances tested, {kept} drawn with Hi-Z, {same}",
            c.visible_entities, c.hiz_tested
        );
        assert!(on == off, "{name}: Hi-Z moved pixels");
    }
    r.quality.gpu_occlusion_culling = true;
    let (scene, statue) = with_dense_statues(scene);
    for (name, at, yaw, pitch) in EYES {
        let cam = Camera::new(at, yaw, pitch);
        let mut view = offscreen(&r);
        shot(&mut r, &mut view, &scene, &cam);
        let (on, c) = shot(&mut r, &mut view, &scene, &cam);
        // The statues' batch is drawn by cluster only when it fit the budget.
        let by_cluster = view.hiz.cameras[0].clusters.ran > 0;
        let skip = if by_cluster { statue } else { 0 };
        let with = triangles(&r, &view, skip) + cluster_indices(&r, &view) / 3;
        r.quality.gpu_cluster_culling = false;
        let mut fresh = offscreen(&r);
        shot(&mut r, &mut fresh, &scene, &cam);
        let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
        let without = triangles(&r, &fresh, 0);
        r.quality.gpu_cluster_culling = true;
        let same = ["PIXELS DIFFER", "same pixels"][usize::from(on == off)];
        println!(
            "{name}: {with} triangles drawn with clusters, {without} without ({} clusters tested, {} culled), {same}",
            c.cluster_tested, c.cluster_culled
        );
        assert!(on == off, "{name}: cluster culling moved pixels");
    }
}

/// The eyes, each a name, a position, and a yaw and pitch.
const EYES: [(&str, Vec3, f32, f32); 4] = [
    (
        "in a room, at a doorway",
        Vec3::new(55.0, 1.6, 52.0),
        0.0,
        0.0,
    ),
    (
        "down a row of doorways",
        Vec3::new(2.0, 1.6, 105.0),
        0.0,
        0.0,
    ),
    (
        "upstairs, at a doorway",
        Vec3::new(105.0, 5.6, 103.0),
        90.0,
        -5.0,
    ),
    (
        "above the level",
        Vec3::new(100.0, 30.0, -20.0),
        90.0,
        -35.0,
    ),
];

/// `scene` with every statue a 64×64-segment sphere split into clusters, one
/// mesh shared by all: the scene and that mesh's index count.
fn with_dense_statues(mut scene: Scene) -> (Scene, u32) {
    let (vertices, mut indices) = primitives::generate_sphere(0.5, 64, 64);
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    let clusters = rusty_asset::clusters::build(&positions, &mut indices, &[]);
    let clusters = Arc::new(clusters.expect("8064 triangles"));
    let ids: Vec<u32> = scene
        .world
        .ids()
        .iter()
        .copied()
        .filter(|&id| scene.world.name(id).is_some_and(|n| *n == "Statue"))
        .collect();
    for id in ids {
        let mut mesh = scene.world.mesh_mut(id).expect("a statue");
        mesh.primitive_type = "Asset".into();
        mesh.asset_ref = Some("test://statue".into());
        (mesh.vertices, mesh.indices) = (vertices.clone(), indices.clone());
        mesh.revision = MeshRevision::next();
        mesh.clusters = Some((mesh.revision, Arc::clone(&clusters)));
    }
    (scene, indices.len() as u32)
}

/// Triangles the base camera's last frame drew by instance, leaving out the
/// batches drawn by cluster (their run is `clustered` indices; 0 for none).
fn triangles(r: &Renderer, view: &RenderView, clustered: u32) -> u32 {
    let cam = &view.hiz.cameras[0];
    let args = words(r, cam.final_args());
    let batches = args.chunks_exact(5).take(cam.drawn_batches as usize);
    batches
        .filter(|a| a[0] != clustered)
        .map(|a| a[0] * a[1] / 3)
        .sum()
}
