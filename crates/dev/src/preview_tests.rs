use super::*;

const MODEL: &str = "engine/models/suzanne.obj";
const SHADER: &str = "engine/shaders/shader.wgsl";

fn checkout_path(rel: &str) -> String {
    rusty_core::project::checkout()
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn unknown_extension_is_a_caller_error_not_an_empty_picture() {
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
    let err = resolve_subject("assets/models/never-imported.fbx").expect_err("no file");
    assert!(err.contains("no such asset"), "{err}");
}

#[test]
fn resolves_each_previewable_kind() {
    assert!(matches!(
        resolve_subject(&checkout_path(MODEL)),
        Ok(PreviewSubject::Model(_))
    ));
    assert!(matches!(
        resolve_subject(&checkout_path(SHADER)),
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
    let out = rusty_test_support::temp::dir()
        .join("rusty-preview-test")
        .join("shader.png");
    let _ = std::fs::remove_file(&out);

    let options = PreviewOptions {
        resolution: 64,
        ..PreviewOptions::default()
    };
    if !capture_asset(&checkout_path(SHADER), &out, options).expect("preview must not error") {
        eprintln!("no GPU adapter — skipping the render half of this test");
        return;
    }

    let image = image::open(&out).expect("a readable PNG landed on disk");
    assert_eq!((image.width(), image.height()), (64, 64));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn gpu_a_centimetre_fbx_quad_previews_as_its_red_and_blue_textures() {
    let fixture =
        rusty_core::project::checkout().join("tests/fixtures/models/embedded_textures.fbx");
    let dir = rusty_test_support::temp::dir();
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
        // Dominant, not pure: the quad's Fresnel sheen of the sky greys both faces.
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
