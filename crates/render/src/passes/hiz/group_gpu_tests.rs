//! The batches of one mesh share a dispatch a stage (#2406): two panels of one
//! mesh, one of them two-sided and so a batch of its own, half behind a wall,
//! are culled by one group's three dispatches and draw what they draw with the
//! cull off.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, words, H, W};
use rusty_components::RenderFace;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_two_batches_of_one_mesh_are_culled_by_one_dispatch_group() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    // One name, one mesh: the second panel's own two-sided material splits the batch.
    panel(
        &mut scene,
        "Panel",
        (Vec3::new(-7.0, -2.0, -5.0), 6.0, Quat::IDENTITY),
    );
    let two = panel(
        &mut scene,
        "Panel",
        (Vec3::new(-3.0, -2.0, -7.0), 6.0, Quat::IDENTITY),
    );
    let key = scene.ensure_material_key(two).unwrap();
    scene.materials.get_mut(&key).unwrap().render_face = RenderFace::Both;
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    let mut frames = Vec::new();
    for _ in 0..6 {
        frames.push(shot(&mut r, &mut view, &scene, &cam));
    }
    let (on, c) = frames.pop().expect("six frames");
    let cull = &view.hiz.cameras[0].clusters;
    assert_eq!(
        (cull.runs_ran, cull.groups_ran),
        (2, 1),
        "two batches, one group"
    );
    assert_eq!(
        cull.dispatched.borrow().len(),
        3,
        "three dispatches a frame"
    );
    let tally = words(&r, cull.tally());
    assert_eq!(
        tally[2..4],
        [2, 1],
        "the copy covers both instances: {tally:?}"
    );
    assert!(
        c.cluster_tested > 0 && c.cluster_culled > 0,
        "the wall culls: {c:?}"
    );
    assert!(c.cluster_culled < c.cluster_tested, "and keeps some: {c:?}");
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(on == off, "the shared group moved pixels");
}
