//! Captured HDR in a probe (#1696): radiance 10 reaches the SH as 10, and a texel
//! past the half float's range never poisons a probe or a cube.

use glam::Vec3;

use super::probe_hdr_tests::{add_box, closed_room, finite, NORMALS, RES};
use crate::ibl::cubemap_capture::{decode_half_texels, HALF_MAX};
use crate::project_cubemap;
use rusty_components::MaterialAsset;

/// A half float's NaN, infinities and negatives are filtered at the capture: one bad
/// texel is black or held at the largest half, never a NaN or infinity downstream.
#[test]
fn a_bad_half_texel_is_filtered_at_the_capture() {
    let texel = |r: u16| [r, 0x3c00, 0xbc00, 0x3c00].map(u16::to_le_bytes).concat();
    let bytes = [0x7e00, 0x7c00, 0xfc00].map(texel).concat();
    let got = decode_half_texels(&bytes);
    assert_eq!(got[0], Vec3::new(0.0, 1.0, 0.0), "NaN is black, -1 is zero");
    assert_eq!(got[1].x, HALF_MAX, "+inf is held at the largest half");
    assert_eq!(got[2].x, 0.0, "-inf is zero");
}

/// Inside a box glowing at radiance 10, every captured texel is 10 and the probe's
/// irradiance is π·10 from every side: the capture keeps HDR light whole, where an
/// 8-bit capture clipped it to 1 (#1696).
#[test]
fn gpu_a_probe_in_a_room_of_radiance_10_keeps_10() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = closed_room([0.0; 3], [10.0; 3]);
    let capture = renderer.capture_light_probe(&scene, Vec3::ZERO, RES, false);
    for face in &capture.cube.faces {
        assert!(
            face.iter().all(|t| (t.x - 10.0).abs() < 0.05),
            "{:?}",
            face[0]
        );
    }
    let sh = project_cubemap(&capture.cube);
    for n in NORMALS {
        let e = sh.eval(n) / std::f32::consts::PI;
        assert!(
            (e - Vec3::splat(10.0)).abs().max_element() < 0.3,
            "{n}: {e}"
        );
    }
}

/// An emissive of 100000 overflows the half float to infinity: the capture holds it
/// at the largest half, so the probe's SH and the reflection cube stay finite.
#[test]
fn gpu_an_emissive_past_the_half_range_stays_finite() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = closed_room([0.5; 3], [0.0; 3]);
    scene.materials.insert(
        "sun".into(),
        MaterialAsset {
            emissive: [100_000.0; 3],
            ..MaterialAsset::default()
        },
    );
    add_box(
        &mut scene,
        "Sun",
        Vec3::new(0.0, 3.0, 2.0),
        Vec3::ONE,
        "sun",
    );
    let capture = renderer.capture_light_probe(&scene, Vec3::ZERO, RES, false);
    let texels = || capture.cube.faces.iter().flatten();
    assert!(texels().all(|t| t.is_finite() && t.max_element() <= HALF_MAX));
    // The forward pass writes at most 60000 (`finite_hdr`, #1699), so a capture never
    // reads an `Inf` to clamp; a sun past that is held there, not dropped.
    assert!(
        texels().any(|t| t.x >= 60000.0),
        "the sun is held, not dropped"
    );
    assert!(finite(&project_cubemap(&capture.cube)));

    let probe = rusty_scene::ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bytes = renderer.bake_reflection(&scene, &probe, RES).bytes;
    let cube = crate::ibl::cubemap::parse_ktx2_cubemap_mips(&bytes)
        .unwrap()
        .decoded();
    let decode = crate::ibl::reflection_prefilter::source::decode_face;
    for level in &cube.mips {
        assert!(
            decode(level).iter().all(|t| t.is_finite()),
            "every mip is finite"
        );
    }
}

/// A reflection probe in an empty scene captures the sky alone, from the HDR target
/// the sky pass draws into: no face is black.
#[test]
fn gpu_a_sky_only_reflection_capture_keeps_the_sky() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let scene = rusty_scene::Scene::new();
    let probe = rusty_scene::ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bytes = renderer.bake_reflection(&scene, &probe, RES).bytes;
    let cube = crate::ibl::cubemap::parse_ktx2_cubemap_mips(&bytes)
        .unwrap()
        .decoded();
    let texels = crate::ibl::reflection_prefilter::source::decode_face(&cube.mips[0]);
    for face in texels.chunks_exact((RES * RES) as usize) {
        let mean = face.iter().sum::<Vec3>() / face.len() as f32;
        assert!(mean.is_finite() && mean.max_element() > 0.01, "{mean}");
    }
}
