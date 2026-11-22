use super::coordinates::{MAP_COORD_RANGE, WORLD_SIZE_RANGE};
use super::field::{no_hint, SurfaceField, SurfaceKind, SurfaceValue};
use super::surface::number;
use super::{fit_data_slot, fit_single_channel_slot, MaterialLibrary};
use rusty_asset::MapRef;
use rusty_components::{DetailLayer, MaterialAsset, UvMapping};

pub use rusty_scene::rules::material::DETAIL_SCALE_RANGE;

/// An image still marked sRGB turns it off: its texels are data.
pub fn set_detail_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    let Some(m) = materials.get_mut(key) else {
        return;
    };
    let map = path.filter(|p| !p.is_empty()).map(MapRef::from);
    if map.is_none() && m.detail.is_none() {
        return;
    }
    fit_data_slot(map.as_ref());
    layer_mut(m).map = map;
}

/// A `Default` image becomes `SingleChannel`: its red is read raw.
pub fn set_detail_mask(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    let Some(m) = materials.get_mut(key) else {
        return;
    };
    let mask = path.filter(|p| !p.is_empty()).map(MapRef::from);
    if mask.is_none() && m.detail.is_none() {
        return;
    }
    fit_single_channel_slot(mask.as_ref(), "a detail mask");
    layer_mut(m).mask = mask;
}

pub fn detail_mask(materials: &MaterialLibrary, key: &str) -> Option<String> {
    let layer = materials.get(key)?.detail.as_ref()?;
    layer.mask.as_ref().map(MapRef::to_string)
}

pub fn clamp_detail(m: &mut MaterialAsset) {
    let Some(layer) = m.detail.as_mut() else {
        return;
    };
    layer.mask = layer.mask.take().filter(|p| *p != "");
    for row in DETAIL_FIELDS {
        if let Ok(v) = row.check((row.get)(m)) {
            (row.put)(m, v);
        }
    }
}

fn layer_mut(m: &mut MaterialAsset) -> &mut DetailLayer {
    m.detail.get_or_insert_with(DetailLayer::default)
}

fn layer(m: &MaterialAsset) -> DetailLayer {
    m.detail.clone().unwrap_or_default()
}

fn pair(value: SurfaceValue) -> [f32; 2] {
    match value {
        SurfaceValue::Vec2(v) => v,
        _ => [0.0; 2],
    }
}

fn has_map(m: &MaterialAsset) -> bool {
    m.detail.as_ref().is_some_and(|d| d.map.is_some())
}

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
    fn a_mask_is_set_read_and_cleared_and_a_default_image_becomes_single_channel() {
        let dir = rusty_test_support::temp::dir();
        let png = dir.join("mask_2258.png");
        image::RgbaImage::new(4, 4).save(&png).unwrap();
        let path = png.to_string_lossy().into_owned();
        let mut materials = one();
        set_detail_mask(&mut materials, "k", Some(String::new()));
        assert_eq!(materials["k"].detail, None, "an empty path writes nothing");
        set_detail_mask(&mut materials, "k", Some(path.clone()));
        assert_eq!(detail_mask(&materials, "k"), Some(path));
        let kind = rusty_asset::sidecar::texture(&png).texture_type;
        assert_eq!(kind, rusty_asset::sidecar::TextureType::SingleChannel);
        set_detail_mask(&mut materials, "k", None);
        assert_eq!(detail_mask(&materials, "k"), None);
        assert!(materials["k"].detail.is_some(), "the layer is kept");
    }

    #[test]
    fn an_empty_path_is_no_map_and_clearing_nothing_writes_nothing() {
        let mut materials = one();
        set_detail_map(&mut materials, "k", Some(String::new()));
        set_detail_map(&mut materials, "k", None);
        assert_eq!(materials["k"].detail, None, "no layer created");
    }

    #[test]
    fn clamp_detail_holds_a_hand_written_layer_to_its_rows() {
        let mut m = MaterialAsset {
            detail: Some(DetailLayer {
                albedo_scale: 9.0,
                normal_scale: f32::NAN,
                world_size: 0.0,
                tiling: [1e9, 8.0],
                ..DetailLayer::default()
            }),
            ..MaterialAsset::default()
        };
        clamp_detail(&mut m);
        let d = m.detail.unwrap();
        assert_eq!((d.albedo_scale, d.normal_scale), (2.0, 0.0));
        assert_eq!((d.world_size, d.tiling), (0.01, [1000.0, 8.0]));
        let mut bare = MaterialAsset::default();
        clamp_detail(&mut bare);
        assert_eq!(bare.detail, None);
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
