//! A baked cube uploaded both ways (#1885): its BC6H blocks as they are, and decoded
//! to `Rgba16Float` as on an adapter without BC. Every mip, the padded 2×2 and 1×1
//! ones included, reads back its own colour through a `texture_cube` either way.

use glam::Vec3;

use crate::render::ibl::cubemap::read_cube_shape;
use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::reflection_prefilter::source::encode_face;
use crate::render::ibl::reflection_prefilter::{mip_count, PrefilteredCubemap};

#[path = "tap_tests.rs"]
mod tap;

/// Mip `level`'s colour: each level its own, so a tap shows which one it read.
fn colour(level: u32) -> Vec3 {
    Vec3::new(0.1 + 0.2 * level as f32, 0.5, 0.8 - 0.15 * level as f32)
}

/// A `size`-texel cube with its full chain, each level solid [`colour`], written to
/// a file named `name`; returns its path.
pub(crate) fn chained_cube(name: &str, size: u32) -> String {
    let mips = (0..mip_count(size))
        .map(|l| {
            let edge = (size >> l).max(1) as usize;
            std::array::from_fn(|_| encode_face(&vec![colour(l); edge * edge]))
        })
        .collect();
    let cube = PrefilteredCubemap {
        mips,
        base_size: size,
    };
    let path = crate::test_temp::dir().join(name);
    crate::core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_every_mip_reads_its_colour_compressed_or_decoded() {
    for decode in [false, true] {
        let Some(mut r) = crate::render::test_gpu::headless_or_skip(4, 4) else {
            return;
        };
        if decode {
            r.reflection_probes.force_uncompressed();
        }
        let path = chained_cube(&format!("upload_{decode}.ktx2"), 16);
        let cube = r.load_cubemap_mips(&path, read_cube_shape(&path).unwrap());
        let cube = cube.unwrap();
        let want = [
            wgpu::TextureFormat::Bc6hRgbUfloat,
            wgpu::TextureFormat::Rgba16Float,
        ];
        assert_eq!(cube.texture.format(), want[usize::from(decode)]);
        let taps: Vec<(Vec3, f32)> = (0..5).map(|l| (Vec3::X, l as f32)).collect();
        for (l, got) in tap::sample(&r, &cube.texture, &taps).iter().enumerate() {
            let want = (colour(l as u32) * 255.0).round();
            let got = Vec3::new(got[0] as f32, got[1] as f32, got[2] as f32);
            let off = (got - want).abs().max_element();
            assert!(off <= 6.0, "decode {decode}, mip {l}: {got} vs {want}");
        }
    }
}
