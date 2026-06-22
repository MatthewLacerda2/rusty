//! Tests for the GGX reflection prefilter (#245). Sibling of `reflection_prefilter.rs`;
//! shares its 300-line source cap. Pure CPU math — no GPU needed.
//!
//! The acceptance: mip0 is a faithful copy of the capture (a mirror), and successive mips
//! are progressively blurrier (higher roughness) than mip0 while keeping the room's mean
//! colour. We measure "blurrier" as lower spatial variance across a face.

use super::super::cube_sample::srgb_to_linear;
use super::*;

/// A capture whose +X face is a sharp half-red / half-black split and the other faces a
/// solid mid-grey — enough high-frequency contrast that the prefilter visibly smooths it.
fn split_capture(res: u32) -> CubemapCapture {
    let mut faces: [Vec<u8>; 6] = Default::default();
    for f in &mut faces {
        let mut buf = vec![0u8; (res * res * 4) as usize];
        for px in buf.chunks_mut(4) {
            px[0] = 80;
            px[1] = 80;
            px[2] = 80;
            px[3] = 255;
        }
        *f = buf;
    }
    // PosX: left half bright red, right half black — a hard edge.
    let posx = &mut faces[CubemapFace::PosX as usize];
    for y in 0..res {
        for x in 0..res {
            let idx = ((y * res + x) * 4) as usize;
            let red = x < res / 2;
            posx[idx] = if red { 255 } else { 0 };
            posx[idx + 1] = 0;
            posx[idx + 2] = 0;
            posx[idx + 3] = 255;
        }
    }
    CubemapCapture {
        resolution: res,
        faces,
    }
}

/// Spatial variance of a face's luminance (a blur lowers it).
fn face_variance(face: &[u8], res: u32) -> f32 {
    let lum: Vec<f32> = (0..(res * res))
        .map(|i| {
            let idx = (i * 4) as usize;
            srgb_to_linear(face[idx])
                + srgb_to_linear(face[idx + 1])
                + srgb_to_linear(face[idx + 2])
        })
        .collect();
    let mean = lum.iter().sum::<f32>() / lum.len() as f32;
    lum.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / lum.len() as f32
}

/// mip0 is an exact copy of the capture (roughness 0 = a perfect mirror).
#[test]
fn mip0_is_a_faithful_copy() {
    let capture = split_capture(16);
    let cube = prefilter(&capture);
    assert_eq!(cube.base_size, 16);
    assert_eq!(cube.level_size(0), 16);
    for face in CubemapFace::ALL {
        assert_eq!(cube.mips[0][face as usize], capture.face(face));
    }
}

/// Higher mips are smoother (lower variance) than mip0 — the roughness blur. We compare a
/// mid roughness mip against the sharp base level on the high-contrast +X face.
#[test]
fn higher_mips_are_blurrier_than_mip0() {
    let capture = split_capture(16);
    let cube = prefilter(&capture);
    assert!(cube.level_count() >= 3, "expected a mip chain");

    let f = CubemapFace::PosX as usize;
    let var0 = face_variance(&cube.mips[0][f], cube.level_size(0));
    let mid = cube.level_count() / 2;
    let var_mid = face_variance(&cube.mips[mid][f], cube.level_size(mid));
    assert!(
        var_mid < var0,
        "expected mip {mid} smoother than mip0: var {var_mid} vs {var0}"
    );
}

/// Roughness selects the mip the shader samples: 0 -> base, 1 -> roughest, linear between.
/// This is the Rust-level test of the mip-selection the forward shader does on the GPU.
#[test]
fn roughness_maps_to_mip_level() {
    // A 16-edge capture yields a 5-deep chain (16,8,4,2,1).
    let cube = prefilter(&split_capture(16));
    let levels = cube.level_count() as u32;
    let last = (levels - 1) as f32;
    assert_eq!(roughness_to_mip(0.0, levels), 0.0);
    assert_eq!(roughness_to_mip(1.0, levels), last);
    assert!((roughness_to_mip(0.5, levels) - last * 0.5).abs() < 1e-6);
    // Out-of-range roughness clamps, never indexing past the chain.
    assert_eq!(roughness_to_mip(2.0, levels), last);
    assert_eq!(roughness_to_mip(-1.0, levels), 0.0);
    assert_eq!(roughness_to_mip(0.5, 8), 3.5);
}

/// The prefilter preserves the room's colour: the +X face stays red-dominant at every mip
/// (the GGX lobe blurs but does not invent colour).
#[test]
fn rough_mip_keeps_room_color() {
    let capture = split_capture(16);
    let cube = prefilter(&capture);
    let f = CubemapFace::PosX as usize;
    for level in 0..cube.level_count() {
        let size = cube.level_size(level);
        let face = &cube.mips[level][f];
        let mut r = 0.0;
        let mut g = 0.0;
        for px in face.chunks(4) {
            r += srgb_to_linear(px[0]);
            g += srgb_to_linear(px[1]);
        }
        let n = (size * size) as f32;
        assert!(
            r / n >= g / n,
            "mip {level}: expected red >= green, got r {} g {}",
            r / n,
            g / n
        );
    }
}
