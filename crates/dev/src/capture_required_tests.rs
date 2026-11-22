use glam::Vec3;

use super::{no_adapter, CaptureHost};
use crate::screenshot;
use rusty_render::{gpu_required, REQUIRE_GPU_ENV};
use rusty_scene::{Camera, Scene};

fn required_here() -> bool {
    std::env::var(REQUIRE_GPU_ENV).as_deref() == Ok("1")
}

#[test]
fn no_adapter_fails_exactly_when_required() {
    assert_eq!(gpu_required(), required_here());
    match no_adapter("draw a test frame") {
        Err(e) => {
            assert!(
                required_here(),
                "no adapter must only fail when required: {e}"
            );
            assert!(
                e.contains(REQUIRE_GPU_ENV) && e.contains("draw a test frame"),
                "{e}"
            );
        }
        Ok(()) => assert!(
            !required_here(),
            "{REQUIRE_GPU_ENV}=1 must turn a skip into Err"
        ),
    }
}

#[test]
fn gpu_capture_renders_when_required() {
    if !required_here() {
        return;
    }
    let mut host = CaptureHost::new();
    let (scene, cam) = (
        Scene::new(),
        Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0),
    );
    let frame = host.draw(&scene, &cam, 8, 8).expect("draw must not error");
    let frame = frame.expect("RUSTY_REQUIRE_GPU=1 but draw produced no frame");
    let ms = frame.cpu_ms;
    assert!(
        0.01 < ms && ms < 1000.0,
        "cpu_ms {ms} is not in milliseconds"
    );
    let path = rusty_test_support::temp::dir().join("rusty_capture_required.png");
    let wrote = screenshot::capture_into(&mut host, &scene, &cam, &path, 8, 8);
    assert_eq!(
        wrote,
        Ok(true),
        "{REQUIRE_GPU_ENV}=1 but capture_into skipped"
    );
    assert!(path.exists(), "the capture must land on disk");
}
