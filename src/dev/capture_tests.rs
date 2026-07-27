//! Tests for [`CaptureHost`] (#355 step 5): one renderer serving N shots of N scenes
//! at N sizes. The single-view renderer could not have done this — the second shot
//! would have come out at the first's size, or with the first scene's pooled
//! resources — so these assertions are the step-5 half of the multi-view guard.
//!
//! GPU tests skip gracefully when no adapter is present (the Linux CI job), so the
//! size/host-state rules that need no device are pinned separately below.

use glam::Vec3;

use super::CaptureHost;
use crate::dev::screenshot;
use crate::render::Camera;
use crate::scene::{DirtyFlag, MeshComponent, Scene};

/// A scene holding one lit box, at `ambient` ambient intensity — the knob the two
/// scenes differ on, because it moves the whole frame's brightness and so survives
/// any framing or resolution difference between the two shots.
fn box_scene(ambient: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = ambient;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = crate::render::gpu::mesh::generate_box(2.0, 2.0, 2.0);
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
    scene
}

fn tmp(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(name)
}

/// An untouched host owns nothing: no device is built until a shot asks for one, so
/// a harness that never screenshots never pays for a renderer (nor a budget slot).
/// Adapter-free — this is the rule Linux CI can still enforce.
#[test]
fn an_unused_host_builds_nothing() {
    let host = CaptureHost::new();
    assert!(
        host.write_png(tmp("rusty_capture_unused.png")).is_err(),
        "with no frame rendered there is nothing to write"
    );
}

/// The step-5 invariant: four shots, two scenes, two sizes, **one** renderer.
///
/// The load-bearing assertion is the last one — after serving a *different scene* at a
/// *different size*, the shared renderer reproduces the first shot **byte for byte**.
/// That is the whole claim of the shared/per-view/per-scene split stated as an
/// equality, with no brightness heuristic to tune: if any per-view state (size, depth,
/// post-FX history) or any per-scene cache (entity pool, shadow maps, skybox binding)
/// leaked between shots, the repeat would differ. The empty-scene shot in between is
/// what makes it non-vacuous, proving scene content actually reaches the image rather
/// than all four frames being the same blank sky.
///
/// One host and one scenario deliberately, not a test per axis: each renderer is a
/// device + pipelines + shadow maps, and on Windows CI's WARP (system RAM) enough
/// concurrent ones exhaust memory mid-suite (#366). Sharing is also the point being
/// made — the whole test is that sharing works.
#[test]
fn one_host_serves_two_scenes_at_two_sizes() {
    let mut host = CaptureHost::new();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    let (boxed, empty) = (box_scene(1.0), Scene::new());

    let first = tmp("rusty_capture_first.png");
    let bare = tmp("rusty_capture_bare.png");
    let resized = tmp("rusty_capture_resized.png");
    let repeat = tmp("rusty_capture_repeat.png");

    let captured = screenshot::capture_into(&mut host, &boxed, &cam, &first, 64, 48)
        .expect("capture must not error");
    if !captured {
        eprintln!("[capture] no GPU/software adapter — skipping the shared-renderer assertion");
        return;
    }
    // A different scene at the same size, then the same scene at a different size —
    // the two axes that used to clobber each other, both exercised between the pair
    // being compared.
    for (scene, path, w, h) in [
        (&empty, &bare, 64, 48),
        (&boxed, &resized, 40, 72),
        (&boxed, &repeat, 64, 48),
    ] {
        assert!(
            screenshot::capture_into(&mut host, scene, &cam, path, w, h)
                .expect("later captures must not error"),
            "every later shot must succeed on the renderer the first one built"
        );
    }

    let read = |p: &std::path::PathBuf| image::open(p).expect("png readable").to_rgb8();
    let (a, d, r, c) = (read(&first), read(&bare), read(&resized), read(&repeat));

    // Each shot landed at its own size — the single-`size` renderer could not have.
    assert_eq!((a.width(), a.height()), (64, 48));
    assert_eq!((r.width(), r.height()), (40, 72));

    // Scene content reaches the image, so the equality below is not vacuous.
    assert_ne!(
        a.as_raw(),
        d.as_raw(),
        "a scene with a box must not render identically to an empty one"
    );

    // The invariant: nothing bled across the intervening scene + resize.
    assert_eq!(
        a.as_raw(),
        c.as_raw(),
        "re-rendering the first scene at its original size through the same renderer \
         must reproduce the first shot exactly"
    );
}
