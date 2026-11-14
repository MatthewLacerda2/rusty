//! Per-cluster culling on lavapipe (#1968): a wall hides half of a big panel and
//! only its visible half is drawn; a panel facing away is dropped by its normal
//! cones, and drawn whole on a two-sided material; pixels match with it off.

use std::sync::Arc;

use glam::{Quat, Vec3};

use super::gpu_tests::{offscreen, shot, words, H, W};
use crate::Renderer;
use rusty_components::mesh::{MeshRevision, Vertex};
use rusty_components::RenderFace;
use rusty_scene::authoring::{create_entity, material, Primitive};
use rusty_scene::lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

/// Quads per side of a test panel: 8192 triangles, about 70 clusters.
const N: u32 = 64;

/// A clustered `N`×`N` panel, `size` metres square, its corner at `at`, facing
/// +Z turned by `turn`.
pub(super) fn panel(scene: &mut Scene, name: &str, (at, size, turn): (Vec3, f32, Quat)) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Plane));
    let mut positions = Vec::new();
    for y in 0..=N {
        for x in 0..=N {
            positions.push([x as f32, y as f32, 0.0]);
        }
    }
    let at_ = |x: u32, y: u32| y * (N + 1) + x;
    let mut indices = Vec::new();
    for y in 0..N {
        for x in 0..N {
            let (a, b, c, d) = (at_(x, y), at_(x + 1, y), at_(x + 1, y + 1), at_(x, y + 1));
            indices.extend([a, b, c, a, c, d]);
        }
    }
    let clusters = rusty_asset::clusters::build(&positions, &mut indices, &[]).expect("big");
    let mut mesh = scene.world.mesh_mut(id).unwrap();
    mesh.primitive_type = "Asset".into();
    mesh.asset_ref = Some(format!("test://{name}"));
    mesh.vertices = positions
        .iter()
        .map(|&p| Vertex::new(Vec3::from_array(p), Vec3::Z, [0.0, 0.0]))
        .collect();
    mesh.indices = indices;
    mesh.revision = MeshRevision::next();
    mesh.clusters = Some((mesh.revision, Arc::new(clusters)));
    drop(mesh);
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation, t.scale) = (at, turn, Vec3::splat(size / N as f32));
    drop(t);
    id
}

/// Indices the base camera's last frame drew by cluster, summed over its jobs.
pub(super) fn cluster_indices(r: &Renderer, view: &crate::RenderView) -> u32 {
    let cull = &view.hiz.cameras[0].clusters;
    let args = words(r, cull.args());
    let jobs = args.chunks_exact(5).take(cull.ran as usize);
    jobs.map(|a| a[0] * a[1]).sum()
}

/// Frames of `scene` from `cam` with cluster culling on (a few, so the
/// history settles) and off: the on frame, its drawn cluster indices, and
/// whether the two frames match.
fn on_and_off(r: &mut Renderer, scene: &Scene, cam: &Camera) -> (Vec<u8>, u32, bool) {
    let mut view = offscreen(r);
    let mut on = Vec::new();
    for _ in 0..3 {
        (on, _) = shot(r, &mut view, scene, cam);
    }
    let drawn = cluster_indices(r, &view);
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(r);
    let (off, _) = shot(r, &mut fresh, scene, cam);
    r.quality.gpu_cluster_culling = true;
    (on.clone(), drawn, on == off)
}

const ALL: u32 = 6 * N * N;

#[test]
fn gpu_a_wall_hides_half_a_panel_and_only_the_visible_half_is_drawn() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    block(
        &mut scene,
        "Wall",
        Vec3::new(-9.0, -3.0, -0.2),
        Vec3::new(0.0, 12.0, 0.2),
    );
    panel(
        &mut scene,
        "Panel",
        (Vec3::new(-6.0, -2.0, -5.0), 12.0, Quat::IDENTITY),
    );
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let (_, half, same) = on_and_off(&mut r, &scene, &cam);
    assert!(same, "cluster culling moved pixels");
    assert!(
        half > ALL / 4 && half < ALL * 3 / 4,
        "about half: {half} of {ALL}"
    );
    // The count reads back a frame or so late; once culling stops it is 0.
    let mut view = offscreen(&r);
    let mut frame = || shot(&mut r, &mut view, &scene, &cam).1;
    let c = (0..9).map(|_| frame()).last().expect("nine frames");
    assert!(c.cluster_tested > 0 && c.cluster_culled > 0, "{c:?}");
    r.quality.gpu_cluster_culling = false;
    let c = shot(&mut r, &mut view, &scene, &cam).1;
    assert_eq!(c.cluster_culled, 0, "stale once culling stopped: {c:?}");
    r.quality.gpu_cluster_culling = true;
    // With nothing in front and all of it on screen, every cluster is drawn.
    let mut open = Scene::new();
    open.skybox_path = String::new();
    panel(
        &mut open,
        "Panel",
        (Vec3::new(-6.0, -2.0, -5.0), 12.0, Quat::IDENTITY),
    );
    let far = Camera::new(Vec3::new(0.0, 4.0, 30.0), -90.0, 0.0);
    let (_, drawn, same) = on_and_off(&mut r, &open, &far);
    assert!(same, "cluster culling moved pixels with nothing hidden");
    assert_eq!(drawn, ALL, "nothing hidden: {half} drawn behind the wall");
}

#[test]
fn gpu_a_panel_facing_away_is_dropped_by_its_cones_unless_two_sided() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let away = Quat::from_rotation_y(std::f32::consts::PI);
    let id = panel(&mut scene, "Back", (Vec3::new(2.0, -1.0, -4.0), 4.0, away));
    let cam = Camera::new(Vec3::new(0.0, 1.0, 6.0), -90.0, 0.0);
    let (one_sided, drawn, same) = on_and_off(&mut r, &scene, &cam);
    assert!(same, "cone culling moved pixels");
    assert_eq!(drawn, 0, "every cluster faces away");
    let key = material::ensure_material_key(&mut scene, id).unwrap();
    material::set_render_face(&mut scene.materials, &key, RenderFace::Both);
    let (two_sided, drawn, same) = on_and_off(&mut r, &scene, &cam);
    assert!(same, "cluster culling moved a two-sided panel's pixels");
    assert_eq!(drawn, ALL, "a two-sided panel has no cone test");
    assert!(two_sided != one_sided, "its back shows");
}
