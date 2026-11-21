use glam::Vec3;

use crate::ibl::cubemap::read_cube_shape;
use crate::ibl::ktx2_encode::encode_cubemap;
use crate::ibl::reflection_prefilter::source::encode_face;
use crate::ibl::reflection_prefilter::{mip_count, PrefilteredCubemap};

use super::super::tap_tests as tap;

fn colour(level: u32) -> Vec3 {
    Vec3::new(0.1 + 0.2 * level as f32, 0.5, 0.8 - 0.15 * level as f32)
}

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
    let path = rusty_test_support::temp::dir().join(name);
    rusty_core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_every_mip_reads_its_colour_compressed_or_decoded() {
    for decode in [false, true] {
        let Some(mut r) = crate::test_gpu::headless_or_skip(4, 4) else {
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
