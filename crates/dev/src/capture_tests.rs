use glam::Vec3;

use super::CaptureHost;
use crate::screenshot;
use rusty_scene::Camera;
use rusty_scene::{MeshComponent, Scene};

fn box_scene(ambient: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    scene.ambient_intensity = ambient;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
}

fn tmp(name: &str) -> std::path::PathBuf {
    rusty_test_support::temp::dir().join(name)
}

#[test]
fn an_unused_host_builds_nothing() {
    let host = CaptureHost::new();
    assert!(
        host.write_png(tmp("rusty_capture_unused.png")).is_err(),
        "with no frame rendered there is nothing to write"
    );
}

#[test]
fn gpu_one_host_serves_two_scenes_at_two_sizes() {
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

    assert_eq!((a.width(), a.height()), (64, 48));
    assert_eq!((r.width(), r.height()), (40, 72));

    assert_ne!(
        a.as_raw(),
        d.as_raw(),
        "a scene with a box must not render identically to an empty one"
    );

    assert_eq!(
        a.as_raw(),
        c.as_raw(),
        "re-rendering the first scene at its original size through the same renderer \
         must reproduce the first shot exactly"
    );
}
