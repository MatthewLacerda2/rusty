use glam::Vec3;

use super::fixtures::{rays, Fixture};
use super::gpu::LIMIT;
use super::probe::trace;
use super::trace_gpu_tests::level;
use crate::Renderer;
use rusty_scene::Scene;

fn refusals(r: &mut Renderer, scene: &Scene) -> usize {
    r.extract_scene(scene);
    r.ray_tree(scene).expect("extracted");
    let logged = rusty_test_support::logs::take();
    logged
        .iter()
        .filter(|m| m.contains("ray-traced tree"))
        .count()
}

#[test]
fn gpu_a_tree_past_the_device_limit_is_refused_whole_and_said_once() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut scene = level();
    let ids = scene.world.ids_with_mesh();
    rusty_test_support::logs::init();
    rusty_test_support::logs::take(); // what earlier code on this thread logged
    LIMIT.set(Some(1024));
    assert_eq!(refusals(&mut r, &scene), 1, "a refusal is said");
    let tree = r.ray_tree(&scene).expect("extracted");
    assert_eq!(tree.gpu_bytes(), 0, "a refused tree holds no bytes");
    assert!(
        tree.packed().tris.len() == 14,
        "the CPU tree is still whole: a plane and a box"
    );
    let mut eyes = Fixture::one(0, glam::Mat4::IDENTITY);
    eyes.eyes = (Vec3::new(-4.0, 0.2, -1.0), Vec3::new(5.0, 3.0, 4.0));
    let rays = rays(&eyes, 200);
    let hits = trace(&mut r, &scene, &rays);
    assert!(
        hits.iter().all(|&(hit, any)| hit.is_none() && !any),
        "every ray misses"
    );
    scene.world.transform_mut(ids[3]).unwrap().position += Vec3::Y;
    assert_eq!(
        refusals(&mut r, &scene),
        0,
        "said once, not on every rebuild"
    );
    LIMIT.set(None);
    scene.world.transform_mut(ids[3]).unwrap().position -= Vec3::Y;
    assert_eq!(refusals(&mut r, &scene), 0);
    let tree = r.ray_tree(&scene).expect("extracted");
    assert!(
        tree.gpu_bytes() > 1024,
        "under the device's own limit it fits again"
    );
    let hits = trace(&mut r, &scene, &rays);
    assert!(hits.iter().any(|(hit, _)| hit.is_some()), "and rays hit it");
}
