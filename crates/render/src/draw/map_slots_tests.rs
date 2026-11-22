use super::*;
use rusty_components::DetailLayer;

fn every_map() -> MaterialAsset {
    MaterialAsset {
        base_color_map: Some("base_color_map".into()),
        metallic_map: Some("metallic_map".into()),
        roughness_map: Some("roughness_map".into()),
        normal_map: Some("normal_map".into()),
        emissive_map: Some("emissive_map".into()),
        detail: Some(DetailLayer {
            map: Some("detail".into()),
            mask: Some("detail_mask".into()),
            ..DetailLayer::default()
        }),
        occlusion_map: Some("occlusion_map".into()),
        specular_map: Some("specular_map".into()),
        height_map: Some("height_map".into()),
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
        (Map::Height, "height_map", 12, TextureKind::Mask),
        (Map::DetailMask, "detail_mask", 11, TextureKind::Mask),
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
    let mut lone = every_map();
    lone.detail.as_mut().unwrap().map = None;
    let paths = material_texture_paths(Some(&lone));
    assert_eq!(
        paths[Map::DetailMask as usize],
        None,
        "no mask without a map"
    );
}

#[test]
fn only_the_maps_that_darken_or_scale_switch_off_while_missing() {
    let off: Vec<Map> = MAP_SLOTS
        .iter()
        .filter(|s| s.off_while_missing.is_some())
        .map(|s| s.map)
        .collect();
    assert_eq!(
        off,
        [
            Map::Detail,
            Map::Occlusion,
            Map::Specular,
            Map::Height,
            Map::DetailMask
        ]
    );
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
        record.level_maps.use_height_map,
        record.detail.use_detail_mask,
    );
    assert_eq!(flags, (7, 7, 7, 7, 7), "each row names its own flag");
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
    let over = MaterialAsset {
        occlusion_strength: 9.0,
        ..m
    };
    let held = level_maps_uniform(Some(&over)).occlusion_strength;
    assert_eq!(held, 1.0, "the strength is read through occlusion_amount");
}

#[test]
fn the_height_map_flags_only_where_it_applies_and_holds_its_depth() {
    let off = level_maps_uniform(None);
    assert_eq!((off.use_height_map, off.height_scale), (0, 0.0));
    let mut m = MaterialAsset {
        height_map: Some("h.png".into()),
        height_scale: 0.5,
        ..MaterialAsset::default()
    };
    let on = level_maps_uniform(Some(&m));
    assert_eq!((on.use_height_map, on.height_scale), (1, 0.2), "clamped");
    m.render_mode = rusty_components::RenderMode::Cutout;
    assert_eq!(level_maps_uniform(Some(&m)).use_height_map, 0, "Cutout");
    m.render_mode = rusty_components::RenderMode::Opaque;
    m.uv_mapping = rusty_components::UvMapping::Triplanar;
    assert_eq!(level_maps_uniform(Some(&m)).use_height_map, 0, "Triplanar");
}
