//! The cluster pool across a session (#2417): a mesh re-cut mid-session gives
//! its rows and indices back and leases new ones (the pool growing for a bigger
//! one), the next re-cut reuses the hole without growing, and every frame
//! between culls by cluster and draws what it draws with the cull off.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::{cut, panel_of};
use super::gpu_tests::{offscreen, shot, H, W};
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
