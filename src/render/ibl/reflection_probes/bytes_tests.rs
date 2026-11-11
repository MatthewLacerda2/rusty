//! `reflection_bytes` (#2085): what the probe array allocates, counted from the
//! texture, spare slots and mips included, and 0 while no cube is resident.

use glam::Vec3;

use super::super::gpu_tests::{cube_file, probes_at, renderer, uncompressed};
use super::super::SPARE_SLOTS;

/// The bytes one `face`² cube takes with `mips` levels in the array's `format`.
fn cube_bytes(face: u32, mips: u32, format: wgpu::TextureFormat) -> u64 {
    let (edge, _) = format.block_dimensions();
    let block = u64::from(format.block_copy_size(None).unwrap());
    (0..mips)
        .map(|m| u64::from((face >> m).max(1).div_ceil(edge)).pow(2) * block * 6)
        .sum()
}

/// No probe, then N probes at one resolution: 0, then N cubes and the spares.
#[test]
fn gpu_reflection_bytes_count_the_allocated_array() {
    for decode in [false, true] {
        let Some(mut r) = (if decode { uncompressed() } else { renderer() }) else {
            return;
        };
        assert_eq!(
            r.reflection_probes.gpu_bytes(),
            0,
            "the stand-in is no probe"
        );
        r.update_reflection_probes(&probes_at(&[]), Vec3::ZERO);
        assert_eq!(r.reflection_probes.gpu_bytes(), 0, "a scene with none");
        let paths: Vec<String> = (0..3)
            .map(|i| cube_file(&format!("bytes_{decode}_{i}.ktx2"), 16, 40 + i))
            .collect();
        let at: Vec<(f32, &str)> = (paths.iter().enumerate())
            .map(|(i, p)| (i as f32 * 4.0, p.as_str()))
            .collect();
        r.update_reflection_probes(&probes_at(&at), Vec3::ZERO);
        let format = r.reflection_probes.format();
        let want = cube_bytes(16, 1, format) * (3 + SPARE_SLOTS) as u64;
        assert_eq!(r.reflection_probes.gpu_bytes(), want, "{format:?}");
        let frame = crate::render::stats::RenderCounters {
            reflection_bytes: r.reflection_probes.gpu_bytes(),
            ..Default::default()
        };
        assert!(frame.pairs().contains(&("reflection_bytes", want)));
    }
}
