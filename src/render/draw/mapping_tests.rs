//! Tiling and Offset pack to what Unity's mean (#1851), without a GPU: the shader's
//! `tile_with` is `uv * uv_st.xy + uv_st.zw`, so a packed record is checked by
//! running that sum on the CPU against Unity's `TRANSFORM_TEX`. And a record's
//! dielectric F0 (#1998), which the shader reads where it held 0.04.

use super::{mapping_uniform, material_data};
use crate::components::{MaterialAsset, UvMapping};

fn mapped(uv_mapping: UvMapping, tiling: [f32; 2], offset: [f32; 2]) -> [f32; 4] {
    let m = MaterialAsset {
        uv_mapping,
        tiling,
        offset,
        ..MaterialAsset::default()
    };
    mapping_uniform(&m).uv_st
}

/// The image row (v from the top, as wgpu samples) the shader reads at mesh `v`.
fn read_at(st: [f32; 4], v: f32) -> f32 {
    v * st[1] + st[3]
}

#[test]
fn uv0_reads_the_texel_unity_reads() {
    let (tiling, offset) = ([2.0, 3.0], [0.25, 0.1]);
    let st = mapped(UvMapping::Uv0, tiling, offset);
    assert_eq!((st[0], st[2]), (2.0, 0.25), "u is Unity's as set");
    for v in [0.0, 0.3, 0.5, 1.0] {
        // Unity: its v runs up the image, so mesh v (down the image) is 1 - v there.
        let unity = (1.0 - v) * tiling[1] + offset[1];
        assert!((1.0 - read_at(st, v) - unity).abs() < 1e-5, "v {v}");
    }
}

#[test]
fn a_positive_y_offset_moves_a_uv0_texture_down_the_surface() {
    // The mesh v (down the surface) where image row 0.4 (from the top) lands, before and after.
    let landing = |oy: f32| {
        let st = mapped(UvMapping::Uv0, [1.0, 1.0], [0.0, oy]);
        (0.4 - st[3]) / st[1]
    };
    assert_eq!(landing(0.0), 0.4, "no offset reads UV0 untouched");
    assert!(landing(0.1) > landing(0.0) + 0.09, "+y slides it down");
}

#[test]
fn defaults_and_world_mappings_pack_as_set() {
    assert_eq!(
        mapped(UvMapping::Uv0, [1.0, 1.0], [0.0, 0.0]),
        [1.0, 1.0, 0.0, 0.0]
    );
    for world in [UvMapping::Planar, UvMapping::Triplanar] {
        // HDRP's planar `uvXZ * _ST.xy + _ST.zw`: an offset moves along the world axis.
        assert_eq!(
            mapped(world, [2.0, 3.0], [0.25, 0.1]),
            [2.0, 3.0, 0.25, 0.1]
        );
    }
}

#[test]
fn a_record_carries_its_dielectric_f0_and_f90_and_none_is_the_default_glass() {
    let f0 = |m: Option<&MaterialAsset>| material_data(true, m).dielectric_reflectance;
    assert_eq!(f0(None), [0.04, 0.04, 0.04, 1.0]);
    assert_eq!(f0(Some(&MaterialAsset::default())), [0.04, 0.04, 0.04, 1.0]);
    let pane = MaterialAsset {
        specular_color: [2.0; 3],
        ..MaterialAsset::default()
    };
    assert_eq!(f0(Some(&pane)), [0.08, 0.08, 0.08, 1.0]);
    // `specular` scales F0 and is the grazing reflectance (#1998).
    let half = MaterialAsset {
        specular: 0.5,
        ..MaterialAsset::default()
    };
    assert_eq!(f0(Some(&half)), [0.02, 0.02, 0.02, 0.5]);
}
