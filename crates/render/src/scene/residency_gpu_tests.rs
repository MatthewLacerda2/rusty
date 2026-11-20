//! crates/render/src/scene/residency_gpu_tests.rs — a load that replaced the World
//! releases the GPU meshes only the scene before used (#2440), keeps the shared
//! ones, releases nothing on Play and Stop, and retries a mesh the pool refused.

use glam::{Quat, Vec3};

use super::super::fixture::frame;
use crate::passes::hiz::cluster_gpu_tests::panel_of;
use crate::{MeshId, Renderer};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

/// A scene of `prims` and one clustered panel cut `n` a side, named `panel`.
fn level(prims: &[Primitive], (panel, n): (&str, u32)) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    for (i, &kind) in prims.iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Prop_{i}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(i as f32 - 1.0, 0.0, -4.0);
    }
    let at = (Vec3::new(-2.0, -2.0, -6.0), 4.0, Quat::IDENTITY);
    panel_of(&mut scene, (panel, n), at);
    scene
}

/// What the panel `name`'s lease holds of the pool, in bytes.
fn leased(r: &Renderer, name: &str) -> u64 {
    let id = MeshId(format!("Asset|test://{name}"));
    let lease = &r.gpu_meshes[&id].clusters.as_ref().expect("leased").lease;
    64 * lease.table.len() as u64 + 4 * lease.indices.len() as u64
}

fn resident(r: &Renderer) -> Vec<String> {
    let mut ids: Vec<String> = r.gpu_meshes.keys().map(|id| id.0.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn gpu_a_load_releases_the_meshes_only_the_scene_before_used() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (_, a) = frame(&mut r, &level(&[Primitive::Box], ("A", 48)));
    assert_eq!(a.mesh_count, 2, "a box and a panel: {:?}", resident(&r));
    assert_eq!(a.cluster_pool_bytes, leased(&r, "A"));
    let (_, b) = frame(&mut r, &level(&[Primitive::Sphere], ("B", 64)));
    assert_eq!(resident(&r), ["Asset|test://B", "Sphere|"], "A's are gone");
    assert_eq!(b.mesh_count, 2);
    let sizes = r.gpu_meshes.values();
    let bytes: u64 = sizes
        .map(|m| m.vertex_buffer.size() + m.index_buffer.size())
        .sum();
    assert_eq!(b.mesh_bytes, bytes);
    assert_eq!(
        b.cluster_pool_bytes,
        leased(&r, "B"),
        "the pool holds B's alone"
    );
}

#[test]
fn gpu_a_mesh_both_scenes_use_is_uploaded_once_and_kept() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    frame(&mut r, &level(&[Primitive::Box], ("A", 48)));
    let next = level(&[Primitive::Box, Primitive::Sphere], ("B", 48));
    let (_, b) = frame(&mut r, &next);
    assert_eq!(b.uploads.count, 2, "the sphere and B's panel, not the box");
    assert_eq!(resident(&r), ["Asset|test://B", "Box|", "Sphere|"]);
    let (_, again) = frame(&mut r, &next);
    assert_eq!((again.uploads.count, again.mesh_count), (0, 3));
}

#[test]
fn gpu_play_and_stop_release_and_upload_nothing_and_hold_the_edit_scene() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut edit = level(&[Primitive::Box], ("A", 48));
    frame(&mut r, &edit);
    edit.set_playing(true);
    let (_, play) = frame(&mut r, &edit);
    assert_eq!((play.uploads.count, play.mesh_count), (0, 2), "Play");
    let mut loaded = level(&[Primitive::Sphere], ("B", 48));
    loaded.set_playing(true);
    let (_, b) = frame(&mut r, &loaded);
    assert_eq!(b.mesh_count, 4, "the edit scene is held while Play runs");
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop draws the held scene's meshes");
    assert_eq!(resident(&r), ["Asset|test://A", "Box|"], "B's are released");
}

#[test]
fn gpu_a_scene_loaded_again_after_another_draws_the_same_pixels() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let first = level(&[Primitive::Box, Primitive::Sphere], ("A", 48));
    let (before, _) = frame(&mut r, &first);
    frame(&mut r, &level(&[Primitive::Plane], ("B", 48)));
    let (after, c) = frame(
        &mut r,
        &level(&[Primitive::Box, Primitive::Sphere], ("A", 48)),
    );
    assert_eq!(c.uploads.count, 3, "uploaded again: {:?}", resident(&r));
    assert!(before == after, "a reload draws what the first load drew");
    assert!(
        before.iter().any(|&b| b != before[0]),
        "something is on screen"
    );
}

#[test]
fn gpu_a_mesh_the_pool_refused_is_retried_when_a_release_frees_room() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    // Room for B's 64×64 panel alone, not beside A's 48×48 one.
    r.hiz.clusters.pool.limit = 4 * 64 * 64 * 6;
    frame(&mut r, &level(&[], ("A", 48)));
    let b = level(&[], ("B", 64));
    let id = MeshId("Asset|test://B".into());
    frame(&mut r, &b);
    assert!(r.retained.residency.refused.is_empty(), "retried");
    assert!(r.gpu_meshes[&id].clusters.is_some(), "B leased once A left");
    assert_eq!(resident(&r), ["Asset|test://B"]);
}
