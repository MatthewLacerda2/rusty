//! FXAA visual test: one aliased scene with the pass on and off; FXAA reaches the image and
//! softens edges, and off is a byte-stable pass-through. Separate file only because test files
//! are capped at 150 lines.

use crate::temp::file as tmp;
use glam::Vec3;
use rusty::components::CameraComponent;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, Scene};

fn scene_with_fxaa(on: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            fov: 60.0,
            far: 100.0,
            fxaa_active: on,
            ..CameraComponent::default()
        }),
    );
    scene
}

/// Count of horizontally adjacent pixel pairs jumping by more than `HARD_STEP` luma. A
/// mean-gradient metric fails: flat sky and box faces bury what FXAA touches (~1% of the mean).
fn hard_steps(path: &std::path::Path) -> usize {
    const HARD_STEP: f64 = 24.0;
    let img = image::open(path).expect("png readable").to_rgb8();
    let luma = |p: &image::Rgb<u8>| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
    let mut steps = 0;
    for y in 0..img.height() {
        for x in 1..img.width() {
            let d = (luma(img.get_pixel(x, y)) - luma(img.get_pixel(x - 1, y))).abs();
            if d > HARD_STEP {
                steps += 1;
            }
        }
    }
    steps
}

#[test]
fn fxaa_softens_edges_and_is_a_pass_through_when_off() {
    // Off-axis, so the silhouette runs diagonally: an axis-aligned view has no aliased edges.
    let cam = Camera::new(Vec3::new(2.5, 1.5, 4.0), -125.0, -18.0);
    let (off, off_again, on) = (
        tmp("rusty_fxaa_off.png"),
        tmp("rusty_fxaa_off_again.png"),
        tmp("rusty_fxaa_on.png"),
    );

    if !capture(&scene_with_fxaa(false), &cam, &off, 128, 128).expect("capture must not error") {
        eprintln!("[fxaa] no GPU/software adapter — skipping visual assertion");
        return;
    }
    for (fxaa, path) in [(false, &off_again), (true, &on)] {
        assert!(capture(&scene_with_fxaa(fxaa), &cam, path, 128, 128).expect("capture"));
    }

    assert_eq!(
        std::fs::read(&off).unwrap(),
        std::fs::read(&off_again).unwrap(),
        "with FXAA off the chain must be a byte-stable pass-through"
    );

    assert_ne!(
        std::fs::read(&off).unwrap(),
        std::fs::read(&on).unwrap(),
        "turning FXAA on must change the rendered image"
    );

    let (s_off, s_on) = (hard_steps(&off), hard_steps(&on));
    eprintln!("[fxaa] hard edge steps: off = {s_off}, on = {s_on}");
    assert!(s_off > 0, "the framing must actually produce aliased edges");
    assert!(
        s_on < s_off,
        "FXAA must ramp hard edges into gradients (off={s_off}, on={s_on})"
    );
}
