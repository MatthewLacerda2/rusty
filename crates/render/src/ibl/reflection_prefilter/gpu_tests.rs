use glam::Vec3;

use super::super::cpu::prefilter;
use super::super::source::encode_face;
use crate::ibl::cube_sample::{face_uv, FaceProjections};
use crate::{CubemapCapture, CubemapFace};
use rusty_core::color::{f16_to_f32, linear_to_srgb};

// The hard capture's bright bar, past 1 so the GPU's HDR path is held too.
const BAR: f32 = 20.0;

// Bounds in sRGB steps of a channel over `BAR`. A GPU's rounding moves a few of 128 taps
// onto their neighbour across a hard edge. Lavapipe: max 1 at every mip.
const MAX_STEP: u8 = 16;
// A face's mean, held from 4×4 up, where a stray tap moves it little. Lavapipe: 0.062.
const MEAN_STEP: f32 = 0.25;
const MEAN_MIN_SIZE: u32 = 4;
// Any mip of the smooth capture: what the CPU reference moves from itself with normals
// nudged 1e-7 to 1e-5. Lavapipe: 1.
const SMOOTH_STEP: u8 = 2;

fn steps(face: &[u8], scale: f32) -> Vec<u8> {
    (face.chunks_exact(2).enumerate())
        .filter(|(i, _)| i % 4 != 3)
        .map(|(_, h)| f16_to_f32(u16::from_le_bytes([h[0], h[1]])) / scale)
        .map(|v| (linear_to_srgb(v) * 255.0 + 0.5) as u8)
        .collect()
}

fn difference(a: &[u8], b: &[u8], scale: f32) -> (u8, f32) {
    assert_eq!(a.len(), b.len(), "face size");
    let (a, b) = (steps(a, scale), steps(b, scale));
    let diffs: Vec<u8> = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).collect();
    let max = diffs.iter().copied().max().unwrap_or(0);
    let mean = diffs.iter().map(|&d| f32::from(d)).sum::<f32>() / diffs.len() as f32;
    (max, mean)
}

fn capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32;
        let texel = |x: u32, y: u32| match (y * 4 / res) == f % 4 {
            true => Vec3::new(1.0, 0.98, 0.94) * BAR,
            false => Vec3::new(
                x as f32 / res.max(2) as f32,
                0.16 * f as f32,
                y as f32 / res as f32,
            ),
        };
        (0..res * res).map(|i| texel(i % res, i / res)).collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(32);
    let cpu = prefilter(&capture);
    let gpu = r.prefilter_gpu(&capture);
    assert_eq!(gpu.base_size, cpu.base_size);
    assert_eq!(gpu.level_count(), cpu.level_count());
    for level in 0..cpu.level_count() {
        let size = capture.resolution >> level;
        let faces = (0..6).map(|f| difference(&cpu.mips[level][f], &gpu.mips[level][f], BAR));
        let (max, mean) = faces.fold((0, 0.0f32), |(m, a), (x, y)| (m.max(x), a.max(y)));
        eprintln!("[prefilter] hard capture, mip {level}: max {max}, worst face mean {mean:.3}");
        assert!(max <= MAX_STEP, "mip {level}: max {max}");
        let held = size < MEAN_MIN_SIZE || mean <= MEAN_STEP;
        assert!(held, "mip {level}: a face's mean {mean:.3}");
    }
}

fn smooth_capture(res: u32) -> CubemapCapture {
    let proj = FaceProjections::new();
    let faces = CubemapFace::ALL.map(|face| {
        let texel = |i: u32| {
            let (u, v) = face_uv(i % res, i / res, res);
            Vec3::splat(0.5) + 0.25 * proj.direction(face, u, v)
        };
        (0..res * res).map(texel).collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference_at_every_mip_of_a_smooth_capture() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = smooth_capture(32);
    let (cpu, gpu) = (prefilter(&capture), r.prefilter_gpu(&capture));
    let step = |l: usize, f: usize| difference(&cpu.mips[l][f], &gpu.mips[l][f], 1.0).0;
    let per_mip: Vec<u8> = (0..cpu.level_count())
        .map(|l| (0..6).map(|f| step(l, f)).max().unwrap_or(0))
        .collect();
    eprintln!("[prefilter] smooth capture, largest step per mip: {per_mip:?}");
    for (level, max) in per_mip.iter().enumerate() {
        assert!(*max <= SMOOTH_STEP, "mip {level}: max {max}");
    }
}

#[test]
fn gpu_one_texel_capture_is_its_own_chain() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(1);
    let gpu = r.prefilter_gpu(&capture);
    assert_eq!(gpu.level_count(), 1);
    assert_eq!(
        gpu.mips[0],
        capture.faces.each_ref().map(|f| encode_face(f))
    );
}
