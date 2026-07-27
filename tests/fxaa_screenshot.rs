//! FXAA visual test (#360). Renders one aliased scene with the pass on and off and
//! asserts (a) FXAA actually reaches the image and softens edges, and (b) with it off
//! the chain is a byte-stable pass-through — the composite writes the output directly,
//! exactly as it did before this pass existed.
//!
//! Its own file rather than an arm of `postfx_screenshot.rs` only because test files
//! are capped at 150 lines; it is the same kind of test, pinning a different knob.
//!
//! When no adapter is available `capture` returns `Ok(false)` and the test passes
//! without asserting — Linux CI has no GPU, so this really runs on macOS/Windows.

#![cfg(feature = "dev")]

use glam::Vec3;
use rusty::components::{CameraComponent, ClearFlags};
use rusty::dev::screenshot::capture;
use rusty::render::Camera;
use rusty::scene::{DirtyFlag, MeshComponent, Scene};

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

/// A lit box plus a camera carrying the FXAA knob. No `VisualCorrection` volume at
/// all, which is itself part of the point: FXAA is read off the camera, so it must
/// work in a scene that has never heard of the post-FX volume.
fn scene_with_fxaa(on: bool) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::render::gpu::mesh::generate_box(2.0, 2.0, 2.0);
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    scene.world.set_camera(
        id,
        Some(CameraComponent {
            active: true,
            fov: 60.0,
            near: 0.1,
            far: 100.0,
            culling_mask: u32::MAX,
            render_order: 0,
            clear_flags: ClearFlags::Skybox,
            motion_blur_active: false,
            motion_blur_samples: 0,
            fxaa_active: on,
        }),
    );
    scene
}

/// How many horizontally adjacent pixel pairs jump by more than `HARD_STEP` luma —
/// i.e. how many *hard* edges the image still has.
///
/// A mean-gradient metric does not work here: the frame is mostly flat sky and flat
/// box faces, so averaging over every pixel buries the handful FXAA actually touches
/// (it moved the mean by ~1%, far too thin to assert across three GPU backends).
/// Counting only the steep transitions measures exactly the staircase FXAA exists to
/// ramp, and moves by a wide margin.
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
    // Off-axis framing so the box's silhouette runs diagonally — the staircase FXAA
    // exists to smooth. An axis-aligned view would have no aliased edges to measure.
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

    // FXAA reaches the image at all — a binary check that no threshold can soften.
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
