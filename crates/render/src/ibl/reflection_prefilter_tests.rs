use glam::Vec3;

use super::cpu::prefilter;
use super::source::{decode_face as texels, encode_face};
use super::*;
use crate::{CubemapCapture, CubemapFace};

fn split_capture(res: u32) -> CubemapCapture {
    let mut faces: [Vec<Vec3>; 6] =
        std::array::from_fn(|_| vec![Vec3::splat(0.08); (res * res) as usize]);
    let red = |i: u32| {
        if i % res < res / 2 {
            Vec3::X
        } else {
            Vec3::ZERO
        }
    };
    faces[CubemapFace::PosX as usize] = (0..res * res).map(red).collect();
    CubemapCapture {
        resolution: res,
        faces,
    }
}

fn face_variance(face: &[u8]) -> f32 {
    let lum: Vec<f32> = texels(face).iter().map(|c| c.element_sum()).collect();
    let mean = lum.iter().sum::<f32>() / lum.len() as f32;
    lum.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / lum.len() as f32
}

#[test]
fn mip0_is_a_faithful_copy() {
    let capture = split_capture(16);
    let cube = prefilter(&capture);
    assert_eq!(cube.base_size, 16);
    assert_eq!(cube.level_size(0), 16);
    for face in CubemapFace::ALL {
        assert_eq!(cube.mips[0][face as usize], encode_face(capture.face(face)));
    }
}

#[test]
fn higher_mips_are_blurrier_than_mip0() {
    let capture = split_capture(16);
    let cube = prefilter(&capture);
    assert!(cube.level_count() >= 3, "expected a mip chain");

    let f = CubemapFace::PosX as usize;
    let var0 = face_variance(&cube.mips[0][f]);
    let mid = cube.level_count() / 2;
    let var_mid = face_variance(&cube.mips[mid][f]);
    assert!(
        var_mid < var0,
        "expected mip {mid} smoother than mip0: var {var_mid} vs {var0}"
    );
}

#[test]
fn roughness_maps_to_mip_level() {
    let cube = prefilter(&split_capture(16));
    let levels = cube.level_count() as u32;
    let last = (levels - 1) as f32;
    assert_eq!(roughness_to_mip(0.0, levels), 0.0);
    assert_eq!(roughness_to_mip(1.0, levels), last);
    assert!((roughness_to_mip(0.5, levels) - last * 0.5).abs() < 1e-6);
    assert_eq!(roughness_to_mip(2.0, levels), last);
    assert_eq!(roughness_to_mip(-1.0, levels), 0.0);
    assert_eq!(roughness_to_mip(0.5, 8), 3.5);
}

#[test]
fn rough_mip_keeps_room_color() {
    let cube = prefilter(&split_capture(16));
    let f = CubemapFace::PosX as usize;
    for level in 0..cube.level_count() {
        let sum: Vec3 = texels(&cube.mips[level][f]).iter().sum();
        assert!(
            sum.x >= sum.y,
            "mip {level}: expected red >= green, got {sum}"
        );
    }
}
