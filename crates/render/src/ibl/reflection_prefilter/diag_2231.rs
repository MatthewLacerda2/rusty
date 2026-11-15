//! TEMPORARY diagnostic for #2231, never committed.

use glam::Vec3;

use super::cpu::{convolve, prefilter, Taps};
use super::mip_count;
use super::source::{encode_face, sample_mip, source_chain};
use crate::ibl::cube_sample::FaceProjections;
use crate::{CubemapCapture, CubemapFace};
use rusty_core::color::{f16_to_f32, linear_to_srgb};

fn steps(face: &[u8], scale: f32) -> Vec<u8> {
    (face.chunks_exact(2).enumerate())
        .filter(|(i, _)| i % 4 != 3)
        .map(|(_, h)| f16_to_f32(u16::from_le_bytes([h[0], h[1]])) / scale)
        .map(|v| (linear_to_srgb(v) * 255.0 + 0.5) as u8)
        .collect()
}

fn maxdiff(a: &[u8], b: &[u8]) -> u8 {
    steps(a, 1.0)
        .iter()
        .zip(steps(b, 1.0))
        .map(|(x, y)| x.abs_diff(y))
        .max()
        .unwrap()
}

pub(crate) fn smooth_capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32 as f32;
        let texel = |x: u32, y: u32| Vec3::new(x as f32, 0.16 * f * res as f32, y as f32);
        (0..res * res)
            .map(|i| texel(i % res, i / res) / res as f32)
            .collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

pub(crate) fn continuous_capture(res: u32) -> CubemapCapture {
    let proj = FaceProjections::new();
    let faces = CubemapFace::ALL.map(|face| {
        (0..res * res)
            .map(|i| {
                let (u, v) = crate::ibl::cube_sample::face_uv(i % res, i / res, res);
                let d = proj.direction(face, u, v);
                Vec3::splat(0.5) + 0.25 * Vec3::new(d.x, d.y, d.z)
            })
            .collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

/// Every mip's max step difference between the CPU reference and itself with each
/// output normal nudged by `eps` (a GPU's rounding, roughly).
fn self_noise(capture: &CubemapCapture, eps: f32) -> Vec<u8> {
    let base = capture.resolution;
    let levels = mip_count(base);
    let proj = FaceProjections::new();
    let chain = source_chain(capture);
    let reference = prefilter(capture);
    let mut out = vec![0];
    for level in 1..levels {
        let size = (base >> level).max(1);
        let roughness = level as f32 / (levels - 1) as f32;
        let mut worst = 0;
        for face in CubemapFace::ALL {
            let texels: Vec<Vec3> = (0..size * size)
                .map(|i| {
                    let (u, v) = crate::ibl::cube_sample::face_uv(i % size, i / size, size);
                    let n = proj.direction(face, u + eps, v + eps * 0.7);
                    convolve(&proj, &chain, base, n, (roughness, Taps::BAKE))
                })
                .collect();
            let nudged = encode_face(&texels);
            worst = worst.max(maxdiff(
                &reference.mips[level as usize][face as usize],
                &nudged,
            ));
        }
        out.push(worst);
    }
    out
}

#[test]
fn gpu_diag_2231_continuous() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        panic!("no adapter");
    };
    for (name, capture) in [
        ("smooth", smooth_capture(32)),
        ("continuous", continuous_capture(32)),
    ] {
        let (cpu, gpu) = (prefilter(&capture), r.prefilter_gpu(&capture));
        let per: Vec<u8> = (0..cpu.level_count())
            .map(|l| {
                (0..6)
                    .map(|f| maxdiff(&cpu.mips[l][f], &gpu.mips[l][f]))
                    .max()
                    .unwrap()
            })
            .collect();
        eprintln!("DIAG2231 {name}: GPU vs CPU max step per mip {per:?}");
    }
    let hard = super::cpu::prefilter(&hard_capture(32));
    let hgpu = r.prefilter_gpu(&hard_capture(32));
    let per: Vec<u8> = (0..hard.level_count())
        .map(|l| {
            (0..6)
                .map(|f| maxdiff_scaled(&hard.mips[l][f], &hgpu.mips[l][f], 20.0))
                .max()
                .unwrap()
        })
        .collect();
    eprintln!("DIAG2231 hard: GPU vs CPU max step per mip {per:?}");
    eprintln!("DIAG2231 adapter {:?}", r.device.adapter_info().name);
    panic!("diagnostic");
}

fn maxdiff_scaled(a: &[u8], b: &[u8], s: f32) -> u8 {
    steps(a, s)
        .iter()
        .zip(steps(b, s))
        .map(|(x, y)| x.abs_diff(y))
        .max()
        .unwrap()
}

fn hard_capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32;
        let texel = |x: u32, y: u32| match (y * 4 / res) == f % 4 {
            true => Vec3::new(1.0, 0.98, 0.94) * 20.0,
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
