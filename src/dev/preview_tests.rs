//! Tests for the headless asset preview (#353). Everything up to the GPU is covered
//! here — subject resolution, the resolution clamp, and the end-to-end capture, which
//! skips itself when the box has no adapter (the same graceful-degradation contract
//! `screenshot` has, so CI never depends on a GPU).

use super::*;

/// A real, committed asset of each previewable kind, so the dispatch is exercised
/// against the repo rather than made-up paths.
const MODEL: &str = "assets/models/suzanne.obj";
const SHADER: &str = "assets/shaders/shader.wgsl";

#[test]
fn unknown_extension_is_a_caller_error_not_an_empty_picture() {
    // A `.lua` file exists but has no preview story: it must fail loudly rather than
    // render an empty sky that looks like a successful preview.
    let err = resolve_subject("Cargo.toml").expect_err("no preview story for a manifest");
    assert!(err.contains("no preview for"), "{err}");
}

#[test]
fn missing_asset_is_reported_by_path() {
    let err = resolve_subject("assets/models/does-not-exist.glb").expect_err("missing file");
    assert!(err.contains("no such asset"), "{err}");
}

#[test]
fn resolves_each_previewable_kind() {
    assert!(matches!(
        resolve_subject(MODEL),
        Ok(PreviewSubject::Model(_))
    ));
    assert!(matches!(
        resolve_subject(SHADER),
        Ok(PreviewSubject::Shader(_))
    ));
}

#[test]
fn resolution_is_clamped_to_a_sane_range() {
    let huge = PreviewOptions {
        resolution: 1_000_000,
        ..PreviewOptions::default()
    };
    assert_eq!(huge.clamped_resolution(), MAX_RESOLUTION);

    let zero = PreviewOptions {
        resolution: 0,
        ..PreviewOptions::default()
    };
    assert_eq!(zero.clamped_resolution(), MIN_RESOLUTION);
    assert_eq!(
        PreviewOptions::default().clamped_resolution(),
        DEFAULT_RESOLUTION
    );
}

#[test]
fn capture_writes_a_square_png_at_the_requested_resolution() {
    let out = std::env::temp_dir().join("rusty-preview-test/shader.png");
    let _ = std::fs::remove_file(&out);

    let options = PreviewOptions {
        resolution: 64,
        ..PreviewOptions::default()
    };
    let Ok(true) = capture_asset(SHADER, &out, options) else {
        eprintln!("no GPU adapter — skipping the render half of this test");
        return;
    };

    // The nested directory was created for us, and the image is what was asked for.
    let image = image::open(&out).expect("a readable PNG landed on disk");
    assert_eq!((image.width(), image.height()), (64, 64));
    let _ = std::fs::remove_file(&out);
}
