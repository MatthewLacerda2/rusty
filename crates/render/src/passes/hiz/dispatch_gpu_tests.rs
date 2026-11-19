//! A frame's cluster cull is one dispatch a stage whatever its meshes (#2417):
//! three panels of unequal cluster counts, each its own mesh, and a two-sided
//! copy of one of them (a batch of its own), half behind a wall, are culled by
//! one test, one scan and one copy dispatch, and draw what they draw with the
//! cull off.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel_of;
use super::gpu_tests::{offscreen, shot, H, W};
use crate::timing::GpuPart;
use rusty_components::RenderFace;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_meshes_of_unequal_cluster_counts_are_culled_by_one_dispatch_a_stage() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let scene = unequal_meshes();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    let frames = (0..6).map(|_| shot(&mut r, &mut view, &scene, &cam));
    let (on, c) = frames.last().expect("six frames");
    let cull = &view.hiz.cameras[0].clusters;
    let (jobs, chunks) = (cull.ran, cull.chunks_ran);
    assert_eq!((cull.runs_ran, jobs), (4, 4), "four batches, a job each");
    assert!(chunks > jobs, "more than a chunk a job: {chunks}");
    let one_each = vec![
        (GpuPart::OcclusionClustersTest, (chunks, 1)),
        (GpuPart::OcclusionClustersScan, (jobs, 1)),
        (GpuPart::OcclusionClustersCopy, (chunks, 1)),
    ];
    assert_eq!(*cull.dispatched.borrow(), one_each, "one dispatch a stage");
    let culled = c.cluster_culled;
    assert!(
        culled > 0 && culled < c.cluster_tested,
        "the wall culls some: {c:?}"
    );
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(on == off, "the one dispatch moved pixels");
}

/// A wall half across panels cut 48, 64 and 96 a side (so unequal cluster
/// counts), each its own mesh, and a two-sided copy of the middle one.
fn unequal_meshes() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    let cuts = [("Small", 48, -8.0), ("Mid", 64, -4.0), ("Big", 96, 0.0)];
    for (name, n, x) in cuts {
        let at = (Vec3::new(x, -2.0, -5.0), 3.5, Quat::IDENTITY);
        panel_of(&mut scene, (name, n), at);
    }
    let at = (Vec3::new(-3.0, -2.0, -7.0), 6.0, Quat::IDENTITY);
    let two = panel_of(&mut scene, ("Mid", 64), at);
    let key = scene.ensure_material_key(two).unwrap();
    scene.materials.get_mut(&key).unwrap().render_face = RenderFace::Both;
    scene
}
