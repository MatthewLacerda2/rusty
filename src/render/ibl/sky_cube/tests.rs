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
