//! Tests for the no-adapter decision (#885): under `RUSTY_REQUIRE_GPU=1` a capture
//! that finds no adapter fails instead of skipping, and a capture that does find one
//! must actually render. Without the variable (a GPU-less dev box) both stay skips.
//!
//! The two halves pin different failures. [`no_adapter_fails_exactly_when_required`]
//! pins the decision itself; [`gpu_capture_renders_when_required`] pins every path
//! *to* it — a `draw` or `capture_into` that wrongly answers "no adapter" never
//! reaches [`no_adapter`], so only a test that insists on a frame can see it.

use glam::Vec3;

use super::{no_adapter, CaptureHost};
use crate::dev::screenshot;
use crate::render::{gpu_required, REQUIRE_GPU_ENV};
use crate::scene::{Camera, Scene};

/// The variable as this process sees it, read independently of [`gpu_required`].
fn required_here() -> bool {
    std::env::var(REQUIRE_GPU_ENV).as_deref() == Ok("1")
}

/// Adapter-free: the decision follows the variable exactly. CI (variable set) proves
/// the `Err` arm and that the dev layer sees the variable at all; a local run without
/// it proves the graceful skip.
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

/// Where a GPU is required, the dev capture layer must produce frames — both the
/// host's `draw` and the screenshot entry point every visual test uses. Without this,
/// a capture path that wrongly reported "no adapter" would skip the whole visual
/// suite and still come back green (the two MISSED mutants of #885).
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
    assert!(
        frame.is_some(),
        "{REQUIRE_GPU_ENV}=1 but draw produced no frame"
    );
    let path = crate::test_temp::dir().join("rusty_capture_required.png");
    let wrote = screenshot::capture_into(&mut host, &scene, &cam, &path, 8, 8);
    assert_eq!(
        wrote,
        Ok(true),
        "{REQUIRE_GPU_ENV}=1 but capture_into skipped"
    );
    assert!(path.exists(), "the capture must land on disk");
}
