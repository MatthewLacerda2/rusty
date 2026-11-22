use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, words, H, W};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_the_copy_starts_a_workgroup_per_chunk_and_writes_the_mesh_s_order() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    let at = (Vec3::new(-6.0, -2.0, -5.0), 12.0, Quat::IDENTITY);
    let id = panel(&mut scene, "Panel", at);
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    let mut stream = || {
        let c = (0..4)
            .map(|_| shot(&mut r, &mut view, &scene, &cam).1)
            .last();
        let cull = &view.hiz.cameras[0].clusters;
        let tally = words(&r, cull.tally());
        let drawn = words(&r, cull.args())[0] as usize;
        let indices = words(&r, cull.indices())[..drawn].to_vec();
        let copy = cull.dispatched.borrow().last().map(|d| d.1);
        let tested = c.expect("four frames").cluster_tested;
        (tested, (tally[0], copy), indices)
    };
    let (tested, (kept, copy), first) = stream();
    let chunks = tested.div_ceil(crate::passes::hiz::clusters::plan::CHUNK);
    assert_eq!(copy, Some((chunks, 1)), "a workgroup a chunk of {tested}");
    assert!(
        kept > 0 && kept < tested * 3 / 4,
        "the wall culls: {kept}/{tested}"
    );
    let (_, _, again) = stream();
    assert_eq!(
        first, again,
        "two frames of one view write one index stream"
    );
    let mesh = scene.world.mesh(id).unwrap().indices.clone();
    let mut tris = mesh.chunks_exact(3);
    for t in first.chunks_exact(3) {
        assert!(tris.any(|m| m == t), "{t:?} out of the mesh's order");
    }
}
