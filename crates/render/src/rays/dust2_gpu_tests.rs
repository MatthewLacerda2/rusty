use glam::Vec3;

use super::probe::measure;
use rusty_lighting::occlusion::bench::level::rooms;
use rusty_scene::Camera;

#[test]
// lint: allow(skip) a measurement on the 6344-mesh level, run by hand, never a gate
#[ignore = "a measurement, run by hand"]
fn gpu_rays_dust2_numbers() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(64, 64) else {
        return;
    };
    let scene = rooms(20);
    r.extract_scene(&scene);
    let eye = Camera::new(Vec3::new(55.0, 1.6, 52.0), 0.0, 0.0);
    let m = measure(&mut r, &scene, &eye, (1920, 1080)).expect("extracted");
    println!("{m:?}");
    let c = m.per_ray;
    println!(
        "a ray: {:.2} instances, {:.1} nodes, {:.1} triangles",
        c.instances, c.nodes, c.tris
    );
    for (row, ms) in [("lean", m.lean_ms), ("full hit", m.full_ms)] {
        let mrays = m.rays_per_second(ms) / 1e6;
        println!(
            "{row}: {ms:.2} ms, {mrays:.0} Mrays/s ({:?} clock)",
            m.clock
        );
    }
    assert!(m.instances > 1000 && m.hit_pixels > 0, "{m:?}");
    let moved = std::time::Instant::now();
    r.retained.scene_mut(scene.id()).unwrap().rays.touch();
    r.ray_tree(&scene);
    println!(
        "top-level rebuild: {:.2} ms",
        moved.elapsed().as_secs_f64() * 1000.0
    );
}
