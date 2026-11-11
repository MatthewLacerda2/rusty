//! A material's detail layer as its record's [`DetailUniform`] (#1997): HDRP Lit's
//! detail map, read by `engine/shaders/forward/detail.wgsl`.

use std::ops::RangeInclusive;

use crate::components::{MaterialAsset, UvMapping};
use crate::render::gpu::uniforms::DetailUniform;
use crate::scene::authoring::material::{DETAIL_SCALE_RANGE, MAP_COORD_RANGE, WORLD_SIZE_RANGE};

/// Where and how strongly material `c`'s detail layer applies (#1997): off (all
/// zero) without a layer or its map. Each value clamped to its authoring row's
/// range, as `uniforms::mapping_uniform` does. Under UV0 its v lane is packed as the main
/// one is (#1851), so a detail Tiling and Offset read Unity's way over the main
/// maps' tiled UV; under a world mapping they apply to the projection as set.
pub(crate) fn detail_uniform(c: &MaterialAsset) -> DetailUniform {
    let Some(d) = c.detail.as_ref().filter(|d| d.map.is_some()) else {
        return DetailUniform::default();
    };
    let within = |v: f32, r: &RangeInclusive<f32>| v.clamp(*r.start(), *r.end());
    let [sx, sy] = d.tiling.map(|v| within(v, &MAP_COORD_RANGE));
    let [ox, oy] = d.offset.map(|v| within(v, &MAP_COORD_RANGE));
    let oy = match c.uv_mapping {
        UvMapping::Uv0 => 1.0 - sy - oy,
        UvMapping::Planar | UvMapping::Triplanar => oy,
    };
    let scale = |v: f32| within(v, &DETAIL_SCALE_RANGE);
    DetailUniform {
        detail_st: [sx, sy, ox, oy],
        use_detail_map: 1,
        detail_world_scale: 1.0 / within(d.world_size, &WORLD_SIZE_RANGE),
        detail_albedo_scale: scale(d.albedo_scale),
        detail_normal_scale: scale(d.normal_scale),
        detail_smoothness_scale: scale(d.smoothness_scale),
        _pad: [0; 3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::DetailLayer;

    fn with(uv_mapping: UvMapping, layer: DetailLayer) -> MaterialAsset {
        MaterialAsset {
            uv_mapping,
            detail: Some(layer),
            ..MaterialAsset::default()
        }
    }

    fn mapped() -> DetailLayer {
        DetailLayer {
            map: Some("d.png".into()),
            tiling: [4.0, 2.0],
            offset: [0.25, 0.1],
            ..DetailLayer::default()
        }
    }

    #[test]
    fn no_layer_or_no_map_is_all_zero() {
        let zero = DetailUniform::default();
        assert_eq!(detail_uniform(&MaterialAsset::default()), zero);
        let mapless = with(UvMapping::Uv0, DetailLayer::default());
        assert_eq!(detail_uniform(&mapless), zero);
    }

    #[test]
    fn uv0_packs_unitys_v_and_a_world_mapping_packs_as_set() {
        let uv0 = detail_uniform(&with(UvMapping::Uv0, mapped()));
        assert_eq!(uv0.use_detail_map, 1);
        assert_eq!(uv0.detail_st, [4.0, 2.0, 0.25, 1.0 - 2.0 - 0.1]);
        let planar = detail_uniform(&with(UvMapping::Planar, mapped()));
        assert_eq!(planar.detail_st, [4.0, 2.0, 0.25, 0.1]);
    }

    #[test]
    fn a_hand_written_layer_is_clamped_to_its_rows_ranges() {
        let wild = DetailLayer {
            world_size: 0.0,
            albedo_scale: f32::INFINITY,
            normal_scale: -1.0,
            smoothness_scale: 3.0,
            ..mapped()
        };
        let u = detail_uniform(&with(UvMapping::Triplanar, wild));
        assert_eq!(u.detail_world_scale, 100.0, "0.01 m at the least");
        let scales = (u.detail_albedo_scale, u.detail_normal_scale);
        assert_eq!((scales, u.detail_smoothness_scale), ((2.0, 0.0), 2.0));
    }
}
