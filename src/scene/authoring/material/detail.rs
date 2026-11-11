//! The detail layer's fields (#1997): HDRP Lit's *Detail Map* inputs, as rows of
//! the material field table, so the Lua adapter (`Material.SetDetailTiling`, …),
//! `DefineAsset`'s recipe and the Inspector take them with no code of their own.
//! The map itself is a path, set by [`set_detail_map`] beside the other maps.
//!
//! A row's write creates the material's [`DetailLayer`] when it has none; a read of
//! a material without one gives the layer's default.
//!
//! Pure.

use std::ops::RangeInclusive;

use super::coordinates::{MAP_COORD_RANGE, WORLD_SIZE_RANGE};
use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use super::surface::number;
use super::{fit_data_slot, MaterialLibrary};
use crate::asset::MapRef;
use crate::components::{DetailLayer, MaterialAsset, UvMapping};

/// HDRP's range for the three detail scales (albedo, normal, smoothness).
pub const DETAIL_SCALE_RANGE: RangeInclusive<f32> = 0.0..=2.0;

/// Set (or, with `None`, clear) material `key`'s detail map. The layer's other
/// fields are kept, so a map swapped back in reads as it did. An image still
/// marked sRGB turns it off (`sidecar::fit_data_slot`): its texels are data.
pub fn set_detail_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        let map = path.map(MapRef::from);
        fit_data_slot(map.as_ref());
        layer_mut(m).map = map;
    }
}

/// `m`'s detail layer, created at its defaults when it has none.
fn layer_mut(m: &mut MaterialAsset) -> &mut DetailLayer {
    m.detail.get_or_insert_with(DetailLayer::default)
}

/// `m`'s detail layer, or the default one.
fn layer(m: &MaterialAsset) -> DetailLayer {
    m.detail.clone().unwrap_or_default()
}

/// A pair the row's `check` clamped.
fn pair(value: SurfaceValue) -> [f32; 2] {
    match value {
        SurfaceValue::Vec2(v) => v,
        _ => [0.0; 2],
    }
}

/// Whether `m` draws a detail map: its rows are inert otherwise.
fn has_map(m: &MaterialAsset) -> bool {
    m.detail.as_ref().is_some_and(|d| d.map.is_some())
}

/// Detail Tiling's hover text: linked to the base's tiled UV under `UV0`, over
/// Detail World Size metres under a world mapping.
fn tiling_hint(m: &MaterialAsset) -> Option<&'static str> {
    Some(match m.uv_mapping {
        UvMapping::Uv0 => {
            "Repeats of the detail per repeat of the main maps (HDRP's Link Details with Base)."
        }
        UvMapping::Planar | UvMapping::Triplanar => {
            "Repeats of the detail across Detail World Size metres of the world projection."
        }
    })
}

/// Every detail-layer field, in Inspector order.
pub static DETAIL_FIELDS: &[SurfaceField] = &[
    SurfaceField {
        key: "detail_tiling",
        api: "DetailTiling",
        label: "Detail Tiling",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: has_map,
        get: |m| SurfaceValue::Vec2(layer(m).tiling),
        hint: tiling_hint,
        put: |m, v| layer_mut(m).tiling = pair(v),
    },
    SurfaceField {
        key: "detail_offset",
        api: "DetailOffset",
        label: "Detail Offset",
        kind: SurfaceKind::Vec2(MAP_COORD_RANGE),
        shown: has_map,
        get: |m| SurfaceValue::Vec2(layer(m).offset),
        hint: no_hint,
        put: |m, v| layer_mut(m).offset = pair(v),
    },
    SurfaceField {
        key: "detail_world_size",
        api: "DetailWorldSize",
        label: "Detail World Size (m)",
        kind: SurfaceKind::Number(WORLD_SIZE_RANGE),
        shown: |m| has_map(m) && m.uv_mapping != UvMapping::Uv0,
        get: |m| SurfaceValue::Number(layer(m).world_size),
        hint: no_hint,
        put: |m, v| layer_mut(m).world_size = number(v),
    },
    SurfaceField {
        key: "detail_albedo_scale",
        api: "DetailAlbedoScale",
        label: "Detail Albedo Scale",
        kind: SurfaceKind::Number(DETAIL_SCALE_RANGE),
        shown: has_map,
        get: |m| SurfaceValue::Number(layer(m).albedo_scale),
        hint: no_hint,
        put: |m, v| layer_mut(m).albedo_scale = number(v),
    },
    SurfaceField {
        key: "detail_normal_scale",
        api: "DetailNormalScale",
        label: "Detail Normal Scale",
        kind: SurfaceKind::Number(DETAIL_SCALE_RANGE),
        shown: has_map,
        get: |m| SurfaceValue::Number(layer(m).normal_scale),
        hint: no_hint,
        put: |m, v| layer_mut(m).normal_scale = number(v),
    },
    SurfaceField {
        key: "detail_smoothness_scale",
        api: "DetailSmoothnessScale",
        label: "Detail Smoothness Scale",
        kind: SurfaceKind::Number(DETAIL_SCALE_RANGE),
        shown: has_map,
        get: |m| SurfaceValue::Number(layer(m).smoothness_scale),
        hint: no_hint,
        put: |m, v| layer_mut(m).smoothness_scale = number(v),
    },
];

#[cfg(test)]
mod tests {
    use super::super::field::{set_surface, surface_field};
    use super::*;

    fn one() -> MaterialLibrary {
        MaterialLibrary::from([("k".to_string(), MaterialAsset::default())])
    }

    #[test]
    fn a_row_creates_the_layer_and_clearing_the_map_keeps_it() {
        let mut materials = one();
        let row = surface_field("DetailNormalScale").expect("a row");
        assert_eq!((row.get)(&materials["k"]), SurfaceValue::Number(1.0));
        set_surface(&mut materials, "k", row, SurfaceValue::Number(9.0)).unwrap();
        let layer = materials["k"].detail.clone().expect("created by the write");
        assert_eq!(
            (layer.normal_scale, layer.map),
            (2.0, None),
            "clamped, no map"
        );
        set_detail_map(&mut materials, "k", Some("d.png".into()));
        set_detail_map(&mut materials, "k", None);
        let layer = materials["k"].detail.clone().unwrap();
        assert_eq!((layer.normal_scale, layer.map), (2.0, None), "kept");
    }

    #[test]
    fn its_rows_show_only_with_a_map_and_world_size_only_under_a_world_mapping() {
        let mut m = MaterialAsset::default();
        assert!(DETAIL_FIELDS.iter().all(|f| !(f.shown)(&m)));
        m.detail = Some(DetailLayer {
            map: Some("d.png".into()),
            ..DetailLayer::default()
        });
        let shown = |m: &MaterialAsset| DETAIL_FIELDS.iter().filter(|f| (f.shown)(m)).count();
        assert_eq!(
            shown(&m),
            DETAIL_FIELDS.len() - 1,
            "no world size under UV0"
        );
        m.uv_mapping = UvMapping::Planar;
        assert_eq!(shown(&m), DETAIL_FIELDS.len());
    }
}
