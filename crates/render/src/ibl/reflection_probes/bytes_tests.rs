use glam::Vec3;

use super::super::gpu_tests::{probes_at, renderer, uncompressed};
use crate::ibl::ktx2_encode::encode_cubemap;
use crate::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::{Camera, Scene};

fn mipped_cube(name: &str, value: u8) -> String {
    let mips = (0..5)
        .map(|m| std::array::from_fn(|_| vec![value; ((16u32 >> m).pow(2) * 8) as usize]))
        .collect();
    let cube = PrefilteredCubemap {
        mips,
        base_size: 16,
    };
    let path = rusty_test_support::temp::dir().join(name);
    rusty_core::fs::written::write(&path, &encode_cubemap(&cube)).unwrap();
    path.to_string_lossy().into_owned()
}

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
