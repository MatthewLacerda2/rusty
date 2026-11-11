//! `reflection_bytes` (#2085): what the probe array allocates, spare slots and
//! mips included, published in a rendered frame's counters, and 0 while no cube is
//! resident.

use glam::Vec3;

use super::super::gpu_tests::{probes_at, renderer, uncompressed};
use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

/// A 16² cube file with its whole chain (16, 8, 4, 2, 1); returns its path.
fn mipped_cube(name: &str, value: u8) -> String {
    let mips = (0..5)
        .map(|m| std::array::from_fn(|_| vec![value; ((16u32 >> m).pow(2) * 8) as usize]))
        .collect();
    let cube = PrefilteredCubemap {
        mips,
        base_size: 16,
    };
    let path = crate::test_temp::dir().join(name);
    crate::core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Render `scene` once and read the `reflection_bytes` the frame published.
fn published(r: &mut Renderer, scene: &Scene) -> u64 {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 4, 4);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    r.render(&mut view, scene, &cam, &out, false);
    let pairs = r.frame_counters.pairs();
    let (_, bytes) = pairs
        .iter()
        .find(|(k, _)| *k == "reflection_bytes")
        .unwrap();
    *bytes
}

/// No probe, then three mipped 16² cubes: 0, then five cubes (three and the two
/// spares) of every mip. Per cube, BC6H is one 16-byte block per 4² texels, at
/// least one a face: (16 + 4 + 1 + 1 + 1) × 16 × 6 = 2208; half floats are 8 bytes
/// a texel: (256 + 64 + 16 + 4 + 1) × 8 × 6 = 16 368.
#[test]
fn gpu_reflection_bytes_count_the_allocated_array() {
    for decode in [false, true] {
        let Some(mut r) = (if decode { uncompressed() } else { renderer() }) else {
            return;
        };
        let mut empty = probes_at(&[]);
        empty.skybox_path = String::new();
        assert_eq!(published(&mut r, &empty), 0, "a scene with none");
        let paths: Vec<String> = (0..3)
            .map(|i| mipped_cube(&format!("bytes_{decode}_{i}.ktx2"), 40 + i))
            .collect();
        let at: Vec<(f32, &str)> = (paths.iter().enumerate())
            .map(|(i, p)| (i as f32 * 4.0, p.as_str()))
            .collect();
        let mut scene = probes_at(&at);
        scene.skybox_path = String::new();
        let per_cube = match r.reflection_probes.format() {
            wgpu::TextureFormat::Bc6hRgbUfloat => 2208,
            wgpu::TextureFormat::Rgba16Float => 16_368,
            other => panic!("no figure for {other:?}"),
        };
        assert_eq!(published(&mut r, &scene), per_cube * 5, "decode {decode}");
    }
}
