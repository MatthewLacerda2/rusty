//! What a frame bakes into the sky reflection cubes (#1553), GPU-free.

use super::{bake_slot, plan, procedural_step, Bake, Prepared};

#[test]
fn the_procedural_sky_is_baked_once_per_settings() {
    assert_eq!(plan(false, None), Bake::Procedural);
    assert_eq!(plan(true, None), Bake::Nothing);
}

/// The first bake fills the bound slot; a later one the other, keeping the sky
/// bound last (#1620).
#[test]
fn a_new_sky_is_baked_beside_the_bound_one() {
    assert_eq!(bake_slot(0, true), 0);
    assert_eq!(bake_slot(0, false), 1);
    assert_eq!(bake_slot(1, false), 0);
}

/// Settings the other slot holds only switch group 0 to it, and bake nothing; the
/// bound slot's settings change nothing; new settings bake beside the bound ones
/// (#1620).
#[test]
fn settings_held_by_the_other_slot_only_rebind() {
    let held = [Some('a'), Some('b')];
    let step = |current, sky| procedural_step(&held, current, &sky);
    let prepared = |baked, switched| Prepared {
        baked,
        rebind: false,
        switched,
    };
    assert_eq!(step(0, 'b'), (1, prepared(false, true)));
    assert_eq!(step(1, 'a'), (0, prepared(false, true)));
    assert_eq!(step(0, 'a'), (0, prepared(false, false)));
    assert_eq!(step(0, 'c'), (1, prepared(true, true)));
    let first = procedural_step(&[None, None], 0, &'a');
    assert_eq!(first, (0, prepared(true, false)), "the first bake");
}

#[test]
fn a_panorama_is_baked_only_when_it_is_not_the_one_held() {
    for procedural_held in [false, true] {
        assert_eq!(plan(procedural_held, Some(false)), Bake::Panorama);
        assert_eq!(plan(procedural_held, Some(true)), Bake::Nothing);
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
