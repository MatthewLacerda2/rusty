//! Tests for the headless asset preview (#353). Everything up to the GPU is covered
//! here — subject resolution, the resolution clamp, and the end-to-end capture, which
//! skips itself when the box has no adapter (the same graceful-degradation contract
//! `screenshot` has, so CI never depends on a GPU).

use super::*;

/// A real, committed asset of each previewable kind, so the dispatch is exercised
/// against the repo rather than made-up paths.
const MODEL: &str = "engine/models/suzanne.obj";
const SHADER: &str = "engine/shaders/shader.wgsl";

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
fn a_converted_source_with_no_cache_is_missing_too() {
    // An exported project previews a `.fbx` from its cache (#1267); with neither, the
    // path is reported as it was.
    let err = resolve_subject("assets/models/never-imported.fbx").expect_err("no file");
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
fn gpu_capture_writes_a_square_png_at_the_requested_resolution() {
    let out = crate::test_temp::dir()
        .join("rusty-preview-test")
        .join("shader.png");
    let _ = std::fs::remove_file(&out);

    let options = PreviewOptions {
        resolution: 64,
        ..PreviewOptions::default()
    };
    // An `Err` (including no adapter under RUSTY_REQUIRE_GPU=1, #885) fails here.
    if !capture_asset(SHADER, &out, options).expect("preview must not error") {
        eprintln!("no GPU adapter — skipping the render half of this test");
        return;
    }

    // The nested directory was created for us, and the image is what was asked for.
    let image = image::open(&out).expect("a readable PNG landed on disk");
    assert_eq!((image.width(), image.height()), (64, 64));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn gpu_a_centimetre_fbx_quad_previews_as_its_red_and_blue_textures() {
    // `embedded_textures.fbx`: a 1 cm quad (the FBX default unit) whose triangles
    // have no colour of their own, only an embedded red texture on one and a blue one
    // on the other. It used to preview as empty sky — a sub-pixel speck, and black
    // besides (#1280) — then all red, its second material dropped (#1285).
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(crate::core::paths::native(
        "tests/fixtures/models/embedded_textures.fbx",
    ));
    let dir = crate::test_temp::dir();
    let fbx = dir.join("quad.fbx");
    std::fs::copy(fixture, &fbx).unwrap();
    let out = dir.join("quad.png");
    let options = PreviewOptions {
        resolution: 64,
        ..PreviewOptions::default()
    };
    if !capture_asset(fbx.to_str().unwrap(), &out, options).expect("preview must not error") {
        eprintln!("no GPU adapter — skipping the render half of this test");
        return;
    }
    let image = image::open(&out).unwrap().to_rgb8();
    let count = |own: usize| {
        // Dominant, not pure: seen near edge-on, the quad's Fresnel sheen of the sky
        // (#1543) greys both faces as it would in Unity.
        let hue = |p: &&image::Rgb<u8>| {
            let others = (0..3).filter(|&c| c != own).map(|c| p[c]).max().unwrap();
            p[own] > 96 && p[own] > others.saturating_add(48)
        };
        image.pixels().filter(hue).count()
    };
    let (red, blue) = (count(0), count(2));
    eprintln!("red={red} blue={blue}");
    assert!(
        red > 64 * 64 / 50 && blue > 64 * 64 / 50,
        "red={red} blue={blue}"
    );
}
