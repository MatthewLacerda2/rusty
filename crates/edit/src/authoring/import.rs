//! crates/edit/src/authoring/import.rs — an imported material as a library material.
//!
//! The model importer's material DTO becomes the scene's [`MaterialAsset`], held by
//! the material ops' rows (`authoring::material`).

use super::material::SurfaceValue;
use rusty_components::RenderFace;
use rusty_scene::MaterialAsset;

/// Convert an imported [`rusty_asset::MaterialData`] (the asset-layer DTO) into a
/// renderable [`MaterialAsset`] for the scene's material library. This is the bridge
/// between the pure-data asset layer and the components layer; it lives in `scene`
/// because only this layer may see both.
///
/// glTF packs metallic + roughness into ONE `metallicRoughnessTexture` (metallic in
/// B, roughness in G), so the single `data.metallic_roughness_map` is mapped to BOTH
/// `metallic_map` AND `roughness_map`; the shader samples `.b` / `.g` from each, so
/// pointing both at the same image reads the correct channel. The base-color alpha is
/// dropped (the renderer's base color is RGB).
pub fn material_asset_from_import(data: &rusty_asset::MaterialData) -> MaterialAsset {
    // Unity's model importer marks a bump map a material names as a Normal map,
    // and its packed metallic/roughness map linear (#1574), so each compresses
    // as what it holds.
    let mut asset = MaterialAsset {
        base_color: [data.base_color[0], data.base_color[1], data.base_color[2]],
        base_color_map: data.base_color_map.clone(),
        metallic: data.metallic,
        metallic_map: data.metallic_roughness_map.clone(),
        roughness: data.roughness,
        roughness_map: data.metallic_roughness_map.clone(),
        normal_map: data.normal_map.clone(),
        emissive: data.emissive,
        emissive_map: data.emissive_map.clone(),
        // glTF's `occlusionTexture` and `KHR_materials_specular`'s `specularTexture`
        // (#2151); an ORM file's occlusion is the metallic-roughness image itself.
        occlusion_map: data.occlusion_map.clone(),
        specular_map: data.specular_map.clone(),
        // A double-sided glTF material draws both faces (#1615), as Unity's glTF
        // importers set *Render Face* to Both.
        render_face: match data.double_sided {
            true => RenderFace::Both,
            false => RenderFace::Front,
        },
        // Imported glTF/OBJ materials default to Opaque (#242): the importer does not
        // yet read glTF `alphaMode`/`alphaCutoff`, so transparency is authored after
        // import via the inspector or the `Material` API.
        ..MaterialAsset::default()
    };
    // `KHR_materials_ior` and `KHR_materials_specular` (#1998), clamped by their rows.
    super::material::import_reflectance(&mut asset, data.ior, data.specular, data.specular_color);
    // Its strength through its row, clamped as an authored one is.
    let strength = SurfaceValue::Number(data.occlusion_strength);
    super::material::import_field(&mut asset, "occlusion_strength", strength);
    super::material::fit_map_slots(&asset);
    asset
}

#[cfg(test)]
mod tests {
    use super::material_asset_from_import;
    use rusty_asset::MaterialData;

    #[test]
    fn import_conversion_fans_mr_map_to_both_channels() {
        let data = MaterialData {
            name: "Bricks".to_string(),
            base_color: [0.8, 0.4, 0.2, 0.5],
            metallic: 0.25,
            roughness: 0.75,
            base_color_map: Some("albedo.png".into()),
            metallic_roughness_map: Some("mr.png".into()),
            normal_map: Some("n.png".into()),
            emissive_map: Some("e.png".into()),
            emissive: [0.1, 0.2, 0.3],
            occlusion_map: Some("mr.png".into()),
            occlusion_strength: 1.5,
            double_sided: false,
            ior: 1.33,
            specular: 0.5,
            specular_map: Some("s.png".into()),
            specular_color: [2.0, 1.0, 0.5],
        };
        let mat = material_asset_from_import(&data);
        assert_eq!(mat.render_face, rusty_components::RenderFace::Front);
        let both = MaterialData {
            double_sided: true,
            ..data.clone()
        };
        let face = material_asset_from_import(&both).render_face;
        assert_eq!(face, rusty_components::RenderFace::Both, "glTF doubleSided");

        // Base-color alpha is dropped; rgb carried through.
        assert_eq!(mat.base_color, [0.8, 0.4, 0.2]);
        assert_eq!(mat.metallic, 0.25);
        assert_eq!(mat.roughness, 0.75);
        assert_eq!(mat.base_color_map, Some("albedo.png".into()));
        // The single glTF MR texture fans out to BOTH engine maps (#203).
        assert_eq!(mat.metallic_map, Some("mr.png".into()));
        assert_eq!(mat.roughness_map, Some("mr.png".into()));
        assert_eq!(mat.normal_map, Some("n.png".into()));
        assert_eq!(mat.emissive, [0.1, 0.2, 0.3]);
        assert_eq!(mat.emissive_map, Some("e.png".into()));
        assert_eq!((mat.ior, mat.specular), (1.33, 0.5));
        assert_eq!(mat.specular_color, [2.0, 1.0, 0.5]);
        // An ORM file's occlusion names the one image; its strength lands clamped.
        assert_eq!(mat.occlusion_map, Some("mr.png".into()));
        assert_eq!(
            mat.occlusion_strength, 1.0,
            "glTF's [0, 1], clamped by the row"
        );
        assert_eq!(mat.specular_map, Some("s.png".into()));
    }

    #[test]
    fn an_imported_normal_map_becomes_a_normal_map() {
        use rusty_asset::sidecar::{texture, TextureType};
        let dir = rusty_test_support::temp::dir();
        let png = dir.join("bumps.png");
        image::RgbaImage::new(4, 4).save(&png).unwrap();
        let data = MaterialData {
            normal_map: Some(png.to_string_lossy().into_owned().into()),
            ..MaterialData::default()
        };
        material_asset_from_import(&data);
        assert_eq!(texture(&png).texture_type, TextureType::NormalMap);
    }

    #[test]
    fn an_imported_metallic_roughness_map_turns_srgb_off() {
        use rusty_asset::sidecar::texture;
        let png = rusty_test_support::temp::dir().join("orm.png");
        image::RgbaImage::new(4, 4).save(&png).unwrap();
        let data = MaterialData {
            metallic_roughness_map: Some(png.to_string_lossy().into_owned().into()),
            ..MaterialData::default()
        };
        material_asset_from_import(&data);
        assert!(
            !texture(&png).srgb,
            "a packed ORM map is data, as Unity marks it"
        );
    }
}
