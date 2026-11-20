//! The instance floor spares a camera drawing by cluster (#2338): one big
//! clustered panel and a wall, two objects, far under the floor, still cull
//! the panel's hidden half by cluster, so a level imported as a few big meshes
//! keeps its cluster cull. With the preset's cluster cull off the same camera
//! is under the floor and skips the stage.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, H, W};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_a_clustered_mesh_under_the_floor_still_culls_by_cluster() {
    let Some(mut r) = crate::test_gpu::culling::floored_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    let at = (Vec3::new(-6.0, -2.0, -5.0), 12.0, Quat::IDENTITY);
    panel(&mut scene, "Panel", at);
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    // The count reads back a frame or so late.
    let c = (0..9)
        .map(|_| shot(&mut r, &mut view, &scene, &cam).1)
        .last();
    let c = c.expect("nine frames");
    assert!(
        c.hiz_tested > 0,
        "two objects, a clustered one: it culls: {c:?}"
    );
    assert!(c.cluster_tested > 0 && c.cluster_culled > 0, "{c:?}");
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(&r);
    let c = shot(&mut r, &mut fresh, &scene, &cam).1;
    assert_eq!(c.hiz_tested, 0, "no cluster cull: under the floor: {c:?}");
    assert!(fresh.hiz.cameras.is_empty(), "it holds no Hi-Z place");
}
