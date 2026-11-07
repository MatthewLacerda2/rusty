//! `TaaParams` against `postfx/taa.wgsl`, no adapter (#1846): the lanes are where
//! the shader reads them, the spans it divides by are positive, and a lens turns a
//! view depth into a distance the way `surface_of` expects.

use glam::{Vec2, Vec3};

use super::*;
use crate::scene::Camera;

fn shader() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/engine/shaders/postfx/taa.wgsl"
    );
    std::fs::read_to_string(path).unwrap()
}

fn camera() -> Camera {
    Camera::new(Vec3::ZERO, 0.0, 0.0)
}

fn params() -> TaaParams {
    let lenses = lenses(&[camera()], 1.0);
    TaaParams::new(
        ((64, 48), (32, 24)),
        (true, Vec2::new(0.25, -0.5), false),
        lenses,
    )
}

#[test]
fn the_lanes_are_where_the_shader_reads_them() {
    let p = params();
    let knobs = [FRAME_INFLUENCE, VARIANCE_CLAMP_SCALE, 1.0, STILL_PIXELS];
    assert_eq!(p.knobs, knobs);
    let rejection = [
        MOVING_FRAME_INFLUENCE,
        MOVING_PIXELS,
        DISOCCLUSION_START,
        DISOCCLUSION_FULL,
    ];
    assert_eq!(p.rejection, rejection);
    let wgsl = shader();
    for read in [
        "(pixels - taa.knobs.w) / (taa.rejection.y - taa.knobs.w)",
        "mix(taa.knobs.x, taa.rejection.x, moving)",
        "taa.rejection.w - taa.rejection.z",
        "(mismatch - taa.rejection.z) / span",
    ] {
        assert!(wgsl.contains(read), "the shader no longer reads `{read}`");
    }
}

#[test]
fn the_struct_and_the_camera_count_match_the_shader() {
    let wgsl = shader();
    let body = wgsl.split("struct TaaParams {").nth(1).unwrap();
    let body = body.split("};").next().unwrap();
    let fields: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && l.contains(':'))
        .map(|l| l.split(':').next().unwrap())
        .collect();
    let rust = ["size", "knobs", "scene", "jitter", "rejection", "lenses"];
    assert_eq!(fields, rust);
    assert!(wgsl.contains(&format!("const MAX_CAMERAS: u32 = {MAX_CAMERAS}u;")));
    assert_eq!(std::mem::size_of::<TaaParams>(), (5 + MAX_CAMERAS) * 16);
}

#[test]
fn the_ramps_the_shader_divides_by_are_positive() {
    // The constants' own order is held at compile time, beside them (`params.rs`).
    let off = params().without(Rules {
        moving: false,
        reveal: false,
    });
    assert_eq!(off.rejection[0], FRAME_INFLUENCE);
    assert!(off.rejection[2] < off.rejection[3], "a span to divide by");
}

#[test]
fn a_lens_is_the_offset_per_ndc_unit() {
    let mut wide = camera();
    wide.fov = 90.0;
    let mut flat = camera();
    flat.projection = Projection::Orthographic { size: 2.0 };
    let [w, f, rest] = [0, 1, 2].map(|i| lenses(&[wide.clone(), flat.clone()], 1.5)[i]);
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5);
    // tan 45° × the aspect across, tan 45° up; perspective.
    assert!(close(w, [1.5, 1.0, 1.0, 0.0]), "{w:?}");
    // The half extents, 2 × 1.5 across and 2 up; orthographic.
    assert!(close(f, [3.0, 2.0, 0.0, 0.0]), "{f:?}");
    assert_eq!(rest, [1.0, 1.0, 1.0, 0.0]);
}
