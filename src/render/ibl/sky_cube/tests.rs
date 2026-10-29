//! What a frame bakes into the sky reflection cubes (#1553), GPU-free.

use super::{plan, Bake};

#[test]
fn the_procedural_sky_is_baked_once() {
    assert_eq!(plan(false, None), Bake::Procedural);
    assert_eq!(plan(true, None), Bake::Nothing);
}

#[test]
fn a_panorama_is_baked_only_when_it_is_not_the_one_held() {
    for procedural_baked in [false, true] {
        assert_eq!(plan(procedural_baked, Some(false)), Bake::Panorama);
        assert_eq!(plan(procedural_baked, Some(true)), Bake::Nothing);
    }
}

#[test]
fn the_cube_has_a_full_mip_chain_at_a_probes_face() {
    assert_eq!(super::SKY_FACE, 128);
    assert_eq!(super::gpu::mip_count(), 8, "128 down to 1");
}

/// The sky cube blurs as a probe's prefilter does (#1553): same sample count and
/// the same roughness per mip, so probe-lit and sky-lit rough metal can't drift.
#[test]
fn each_mip_is_filtered_as_a_probes_is() {
    use crate::render::ibl::reflection_prefilter::{roughness_to_mip, SAMPLE_COUNT};
    let mips = super::gpu::mip_count();
    for level in 0..mips {
        let [roughness, size, _, samples] = super::gpu::params(level, true);
        assert_eq!(samples, SAMPLE_COUNT as f32);
        assert_eq!(size, (super::SKY_FACE >> level) as f32);
        assert!((roughness_to_mip(roughness, mips) - level as f32).abs() < 1e-5);
    }
    assert_eq!(super::gpu::params(0, false)[2], 0.0, "procedural source");
}
