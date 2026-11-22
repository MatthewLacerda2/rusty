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
    assert!(m.instances > 1000 && m.hit_pixels > 0, "{m:?}");
    let moved = std::time::Instant::now();
    r.retained.scene_mut(scene.id()).unwrap().rays.touch();
    r.ray_tree(&scene);
    println!(
        "top-level rebuild: {:.2} ms",
        moved.elapsed().as_secs_f64() * 1000.0
    );
}
