//! The caster array's layout, pinned against the shadow stage's WGSL.

use super::CasterData;

/// `CasterData` against `shader.wgsl`'s `Caster`, its mapping terms included (#1594).
#[test]
fn the_caster_layout_matches_the_shadow_stage() {
    use crate::render::gpu::uniform_layout_tests::{assert_members, wgsl_struct_in};
    use std::mem::{offset_of, size_of};
    let shader = std::fs::read_to_string("engine/shaders/shader.wgsl").unwrap();
    let pairs = [
        ("bone_base", offset_of!(CasterData, bone_base)),
        ("alpha_cutoff", offset_of!(CasterData, alpha_cutoff)),
        ("use_texture", offset_of!(CasterData, use_texture)),
        ("terms", offset_of!(CasterData, terms)),
    ];
    let found = wgsl_struct_in(&shader, "Caster");
    assert_members(found, "Caster", &pairs, size_of::<CasterData>());
}
