//! The cluster pool across a session (#2417): a mesh re-cut mid-session gives
//! its rows and indices back and leases new ones (the pool growing for a bigger
//! one), the next re-cut reuses the hole without growing, and every frame
//! between culls by cluster and draws what it draws with the cull off. The
//! limit is the session's: a lease past it is refused with nothing kept, a
//! smaller one still fits, and a mesh refused is drawn whole.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::{cut, panel, panel_of};
use super::gpu_tests::{offscreen, shot, H, W};
use crate::passes::hiz::clusters::pool::ClusterPool;
use crate::Renderer;
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

#[test]
fn gpu_a_mesh_re_cut_mid_session_frees_its_range_and_the_next_cut_reuses_it() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    let at = |x| (Vec3::new(x, -2.0, -5.0), 5.0, Quat::IDENTITY);
    let a = panel_of(&mut scene, ("A", 64), at(-6.0));
    let b = panel_of(&mut scene, ("B", 48), at(-1.0));
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let live = |scene: &Scene| {
        let of = |id| scene.world.mesh(id).unwrap().clusters().unwrap().list.len() as u32;
        let indices = |id| scene.world.mesh(id).unwrap().indices.len() as u32;
        (of(a) + of(b), indices(a) + indices(b))
    };
    settles_culled(&mut r, &scene, &cam);
    let (first, bytes) = r.hiz.clusters.pool.held();
    assert_eq!(first, live(&scene), "both meshes leased");
    // Bigger: a new upload, the old one's lease freed, the pool grown for it.
    cut(&mut scene, a, ("A", 96));
    settles_culled(&mut r, &scene, &cam);
    let (held, grown) = r.hiz.clusters.pool.held();
    assert_eq!(held, live(&scene), "only the live leases are held");
    assert!(
        grown.0 > bytes.0 && grown.1 > bytes.1,
        "{bytes:?} -> {grown:?}"
    );
    // Back: written in place, the 96 cut's lease freed and the first hole reused.
    cut(&mut scene, a, ("A", 64));
    settles_culled(&mut r, &scene, &cam);
    assert_eq!(
        r.hiz.clusters.pool.held(),
        (first, grown),
        "reused, not grown"
    );
}

#[test]
fn gpu_a_lease_past_the_pool_limit_is_refused_and_a_smaller_one_still_fits() {
    let Some(r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    let at = (Vec3::ZERO, 1.0, Quat::IDENTITY);
    let (big, small) = (
        panel_of(&mut scene, ("Big", 64), at),
        panel_of(&mut scene, ("Small", 48), at),
    );
    let mesh = |id| {
        let m = scene.world.mesh(id).unwrap();
        (m.clusters().unwrap().clone(), m.indices.clone())
    };
    let ((bc, bi), (sc, si)) = (mesh(big), mesh(small));
    let mut pool = ClusterPool::new(&r.device, &r.hiz.clusters.pool_layout);
    // Room for one big mesh's indices and one small one's, no more.
    pool.limit = 4 * (bi.len() + si.len()) as u64;
    let gpu = (&r.device, &r.queue);
    let layout = &r.hiz.clusters.pool_layout;
    let first = pool.lease(gpu, layout, (&bc, &bi)).expect("the first fits");
    let held = pool.held().0;
    assert!(
        pool.lease(gpu, layout, (&bc, &bi)).is_none(),
        "a second big one does not"
    );
    assert_eq!(pool.held().0, held, "the refusal kept nothing");
    let second = pool
        .lease(gpu, layout, (&sc, &si))
        .expect("a smaller one fits");
    assert_eq!(second.indices.start, bi.len() as u32, "after the first");
    drop(first);
    let again = pool
        .lease(gpu, layout, (&bc, &bi))
        .expect("the freed range fits it");
    assert_eq!(
        (again.table.start, again.indices.start),
        (0, 0),
        "the hole reused"
    );
}

#[test]
fn gpu_a_mesh_the_pool_refuses_is_drawn_whole() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    panel(
        &mut scene,
        "A",
        (Vec3::new(-2.0, -2.0, -5.0), 4.0, Quat::IDENTITY),
    );
    r.hiz.clusters.pool.limit = 0;
    let cam = Camera::new(Vec3::new(0.0, 0.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    let frames: Vec<_> = (0..4)
        .map(|_| shot(&mut r, &mut view, &scene, &cam))
        .collect();
    let (on, c) = frames.last().expect("four frames");
    assert_eq!(c.cluster_tested, 0, "no lease, no cluster test: {c:?}");
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(*on == off, "drawn whole, as with the cull off");
    assert!(on.iter().any(|&b| b != on[0]), "the panel is on screen");
}

/// A few frames of `scene` with the cull on, which must cull by cluster and
/// draw what one with it off draws.
fn settles_culled(r: &mut Renderer, scene: &Scene, cam: &Camera) {
    let mut view = offscreen(r);
    let frames = (0..6).map(|_| shot(r, &mut view, scene, cam));
    let (on, c) = frames.last().expect("six frames");
    assert!(c.cluster_culled > 0, "the wall culls clusters: {c:?}");
    r.quality.gpu_cluster_culling = false;
    let mut fresh = offscreen(r);
    let (off, _) = shot(r, &mut fresh, scene, cam);
    r.quality.gpu_cluster_culling = true;
    assert!(on == off, "the pool moved pixels");
}
