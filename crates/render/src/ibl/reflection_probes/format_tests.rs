use glam::Vec3;

use super::super::gpu_tests::{probes_at, renderer, uncompressed};
use super::array_bytes;
use crate::ibl::upload::chained_cube;

#[test]
fn bc6h_cubes_fit_eight_times_as_many_in_the_budget() {
    use super::super::{MAX_REFLECTION_PROBES, SPARE_SLOTS};
    use wgpu::TextureFormat::{Bc6hRgbUfloat, Rgba16Float};
    let budget = rusty_scene::lighting::settings::ReflectionSettings::default().budget_bytes();
    let resident = |format| {
        let fit = (budget / array_bytes(512, 1, format)) as usize;
        (fit, (fit - SPARE_SLOTS).min(MAX_REFLECTION_PROBES))
    };
    assert_eq!(array_bytes(512, 1, Rgba16Float), 16 << 20);
    assert_eq!(array_bytes(512, 1, Bc6hRgbUfloat), 2 << 20);
    assert_eq!(array_bytes(4, 1, Bc6hRgbUfloat), 128, "one block a face");
    assert_eq!(resident(Rgba16Float), (16, 14));
    assert_eq!(resident(Bc6hRgbUfloat), (128, 32));
}

#[test]
fn gpu_mipped_cubes_are_resident_compressed_or_decoded() {
    for decode in [false, true] {
        let Some(mut r) = (if decode { uncompressed() } else { renderer() }) else {
            return;
        };
        let compressed = r
            .device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let (big, small) = (
            chained_cube(&format!("array_16_{decode}.ktx2"), 16),
            chained_cube(&format!("array_4_{decode}.ktx2"), 4),
        );
        let scene = probes_at(&[(-2.0, &big), (2.0, &small)]);
        r.update_reflection_probes(&scene, Vec3::ZERO);
        let probes = &r.reflection_probes;
        let want = match compressed && !decode {
            true => wgpu::TextureFormat::Bc6hRgbUfloat,
            false => wgpu::TextureFormat::Rgba16Float,
        };
        assert_eq!((probes.format(), probes.texture.format()), (want, want));
        assert_eq!(probes.copies, 2, "both cubes copied in");
        for x in [-2.0, 2.0] {
            assert_ne!(
                probes.probe_for(Vec3::new(x, 0.0, 0.0)),
                0,
                "{decode} at {x}"
            );
        }
    }
}
