//! The array's cube format (#1885): BC6H where the adapter has BC, half floats
//! decoded at load where it doesn't, and what each costs the budget.

use glam::Vec3;

use super::super::gpu_tests::{probes_at, renderer, uncompressed};
use super::array_bytes;
use crate::ibl::upload::chained_cube;

/// A 512-texel cube with its mips is 16 MiB in half floats (#1696) and 2 MiB in
/// BC6H (#1885): the default 256 MB budget held 14 such cubes and two spares; it
/// now pays for 128, so the 32 the array shades at most (its layer cap,
/// `MAX_REFLECTION_PROBES`) are all resident.
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

/// Two full-chain cubes, 16 and 4 texels (the smaller lands two mips down, its
/// 2×2 and 1×1 mips one padded block each), become resident in the array's format
/// on both paths: BC6H where the adapter has it, half floats when forced.
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
