use super::*;
use rusty_components::DetailLayer;

/// A material naming every map, each by its own field's name.
fn every_map() -> MaterialAsset {
    MaterialAsset {
        base_color_map: Some("base_color_map".into()),
        metallic_map: Some("metallic_map".into()),
        roughness_map: Some("roughness_map".into()),
        normal_map: Some("normal_map".into()),
        emissive_map: Some("emissive_map".into()),
        detail: Some(DetailLayer {
            map: Some("detail".into()),
            ..DetailLayer::default()
        }),
        occlusion_map: Some("occlusion_map".into()),
        specular_map: Some("specular_map".into()),
        ..MaterialAsset::default()
    }
}

#[test]
fn each_row_reads_its_own_field_at_its_own_binding() {
    let paths = material_texture_paths(Some(&every_map()));
    let expected = [
        (Map::Albedo, "base_color_map", 0, TextureKind::Colour),
        (Map::Metallic, "metallic_map", 2, TextureKind::Mask),
        (Map::Roughness, "roughness_map", 3, TextureKind::Mask),
        (Map::Normal, "normal_map", 4, TextureKind::Normal),
        (Map::Emissive, "emissive_map", 5, TextureKind::Colour),
        (Map::Detail, "detail", 15, TextureKind::Data),
        (Map::Occlusion, "occlusion_map", 14, TextureKind::Mask),
        (Map::Specular, "specular_map", 13, TextureKind::Mask),
    ];
    assert_eq!(expected.len(), MATERIAL_MAPS);
    for (map, field, binding, kind) in expected {
        let i = map as usize;
        assert_eq!(
            paths[i].as_ref().map(MapRef::as_str),
            Some(field),
            "{map:?}"
        );
        let row = &MAP_SLOTS[i];
        assert_eq!((row.map, row.binding, row.kind), (map, binding, kind));
        assert_eq!(map_kind(i), kind);
    }
    assert_eq!(map_kind(MATERIAL_MAPS), TextureKind::Data, "an extra slot");
}

#[test]
fn only_the_maps_that_darken_or_scale_switch_off_while_missing() {
    let off: Vec<Map> = MAP_SLOTS
        .iter()
        .filter(|s| s.off_while_missing.is_some())
        .map(|s| s.map)
        .collect();
    assert_eq!(off, [Map::Detail, Map::Occlusion, Map::Specular]);
    let mut record = MaterialData::flat([1.0; 4]);
    for s in &MAP_SLOTS {
        if let Some(flag) = s.off_while_missing {
            *flag(&mut record) = 7;
        }
    }
    let flags = (
        record.detail.use_detail_map,
        record.level_maps.use_occlusion_map,
        record.level_maps.use_specular_map,
    );
    assert_eq!(flags, (7, 7, 7), "each row names its own flag");
}

#[test]
fn the_level_maps_flag_what_is_named_and_hold_the_strength_in_range() {
    let off = level_maps_uniform(None);
    assert_eq!((off.use_occlusion_map, off.occlusion_strength), (0, 1.0));
    let m = MaterialAsset {
        occlusion_map: Some("ao.png".into()),
        occlusion_strength: 0.25,
        ..MaterialAsset::default()
    };
    let on = level_maps_uniform(Some(&m));
    let flags = (
        on.use_occlusion_map,
        on.occlusion_strength,
        on.use_specular_map,
    );
    assert_eq!(flags, (1, 0.25, 0));
    assert_eq!(
        occlusion_strength(f32::NAN),
        1.0,
        "a NaN applies the map whole"
    );
    let ends = (occlusion_strength(-2.0), occlusion_strength(9.0));
    assert_eq!(ends, (0.0, 1.0));
}
