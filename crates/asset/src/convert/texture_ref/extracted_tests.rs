use super::extracted::{glb_of, images, model_source, of_slots, settings};
use crate::convert::glb::{container, image_slots, Slot};
use crate::convert::textures_owner;
use crate::sidecar::TextureType;
use std::path::Path;

#[test]
fn the_slot_decides_the_texture_type() {
    let typed = |slots: &[Slot]| {
        let s = of_slots(slots);
        (s.texture_type, s.srgb)
    };
    assert_eq!(typed(&[Slot::Normal]), (TextureType::NormalMap, true));
    assert_eq!(typed(&[Slot::Red]), (TextureType::SingleChannel, true));
    assert_eq!(typed(&[Slot::Data]), (TextureType::Default, false));
    assert_eq!(typed(&[Slot::Colour]), (TextureType::Default, true));
    assert_eq!(
        typed(&[Slot::Red, Slot::Data]),
        (TextureType::Default, false),
        "an ORM map is packed data"
    );
    assert_eq!(
        typed(&[Slot::Normal, Slot::Colour]),
        (TextureType::Default, true),
        "two kinds take the one every slot reads whole"
    );
    assert_eq!(
        typed(&[]),
        (TextureType::Default, true),
        "unused: the defaults"
    );
}

/// A `.glb` of one material sampling `orm.png` as occlusion and
/// metallic-roughness, `ao.png` as occlusion alone and `n.png` as its normal map.
const GLTF: &str = r#"{"asset":{"version":"2.0"},
 "images":[{"uri":"m.fbx.glb.0123456789abcdef.textures/orm.png"},
           {"uri":"m.fbx.glb.0123456789abcdef.textures/ao.png"},
           {"uri":"m.fbx.glb.0123456789abcdef.textures/n.png"}],
 "textures":[{"source":0},{"source":1},{"source":2}],
 "materials":[{"pbrMetallicRoughness":{"metallicRoughnessTexture":{"index":0}},
               "occlusionTexture":{"index":0},"normalTexture":{"index":2}},
              {"occlusionTexture":{"index":1}}]}"#;

#[test]
fn a_glbs_slots_are_read_by_image() {
    let dir = rusty_test_support::temp::dir();
    let cache = dir.join(crate::convert::IMPORT_CACHE_DIR).join("assets");
    std::fs::create_dir_all(&cache).unwrap();
    let glb = cache.join("m.fbx.glb");
    std::fs::write(&glb, container(GLTF.as_bytes().to_vec(), Vec::new())).unwrap();
    let slots = image_slots(&glb).unwrap();
    let at = |name: &str| format!("m.fbx.glb.0123456789abcdef.textures/{name}");
    assert_eq!(slots[&at("orm.png")], [Slot::Data, Slot::Red]);
    assert_eq!(slots[&at("ao.png")], [Slot::Red]);
    let folder = cache.join("m.fbx.glb.0123456789abcdef.textures");
    let typed = |name: &str| settings(&folder.join(name)).map(|s| s.texture_type);
    assert_eq!(typed("ao.png"), Some(TextureType::SingleChannel));
    assert_eq!(typed("n.png"), Some(TextureType::NormalMap));
    assert_eq!(typed("orm.png"), Some(TextureType::Default));
    assert_eq!(typed("unused.png"), Some(TextureType::Default));
    assert_eq!(
        model_source(&folder.join("ao.png")),
        Some(dir.join("assets/m.fbx"))
    );
}

#[test]
fn only_an_image_in_a_cached_models_textures_folder_is_extracted() {
    assert_eq!(
        textures_owner("m.fbx.glb.0123456789abcdef.textures"),
        Some("m.fbx.glb")
    );
    assert_eq!(textures_owner("m.fbx.glb.textures"), Some("m.fbx.glb"));
    assert_eq!(textures_owner("m.fbx.glb.xyz.textures"), None);
    assert_eq!(textures_owner("photos.textures"), None);
    let cached = Path::new("assets/generated/imported/assets/m.fbx.glb.textures/a.png");
    assert_eq!(
        glb_of(cached),
        Some(Path::new("assets/generated/imported/assets/m.fbx.glb").into())
    );
    let authored = Path::new("assets/m.fbx.glb.textures/a.png");
    assert_eq!(glb_of(authored), None, "outside the import cache");
    assert_eq!(settings(authored), None);
}

#[test]
fn a_packed_specular_map_is_data_and_its_colour_twin_is_never_encoded() {
    let dir = rusty_test_support::temp::dir();
    let folder = dir
        .join(crate::convert::IMPORT_CACHE_DIR)
        .join("m.fbx.glb.textures");
    std::fs::create_dir_all(&folder).unwrap();
    let gltf = r#"{"asset":{"version":"2.0"},
     "images":[{"uri":"m.fbx.glb.textures/spec.png"},{"uri":"m.fbx.glb.textures/tint.png"}],
     "textures":[{"source":0},{"source":1}],
     "materials":[{"extensions":{"KHR_materials_specular":
        {"specularTexture":{"index":0},"specularColorTexture":{"index":1}}}}]}"#;
    let glb = folder.with_file_name("m.fbx.glb");
    std::fs::write(&glb, container(gltf.as_bytes().to_vec(), Vec::new())).unwrap();
    for name in ["spec.png", "tint.png"] {
        std::fs::write(folder.join(name), rusty_test_support::models::png(4, 0)).unwrap();
    }
    assert_eq!(image_slots(&glb).unwrap().len(), 1, "only spec.png is read");
    assert_eq!(
        images(&glb),
        [folder.join("spec.png")],
        "tint.png is not encoded"
    );
    crate::texture_import::import_in_background();
    crate::texture_import::queue_extracted(&glb);
    assert_eq!(
        crate::texture_import::background::take_queued(),
        [folder.join("spec.png")]
    );
    let warned = rusty_test_support::logs::during(|| {
        for _ in 0..2 {
            assert_eq!(settings(&folder.join("tint.png")), Some(Default::default()));
        }
    });
    assert_eq!(warned.len(), 1, "once, then remembered: {warned:?}");
    assert!(warned[0].contains("does not read"), "{warned:?}");
}

#[test]
fn a_noted_conversion_re_reads_the_slots_and_a_miss_is_warned_of() {
    let dir = rusty_test_support::temp::dir();
    let cache = dir.join(crate::convert::IMPORT_CACHE_DIR);
    let folder = cache.join("m.fbx.glb.textures");
    std::fs::create_dir_all(&folder).unwrap();
    let glb = cache.join("m.fbx.glb");
    let write = |slot: &str| {
        let gltf = format!(
            r#"{{"asset":{{"version":"2.0"}},"images":[{{"uri":"m.fbx.glb.textures/n.png"}}],
             "textures":[{{"source":0}}],"materials":[{{{slot}:{{"index":0}}}}]}}"#
        );
        std::fs::write(&glb, container(gltf.into_bytes(), Vec::new())).unwrap();
    };
    let typed = |name: &str| settings(&folder.join(name)).map(|s| s.texture_type);
    write(r#""normalTexture""#);
    assert_eq!(typed("n.png"), Some(TextureType::NormalMap));
    write(r#""emissiveTexture""#);
    rusty_core::fs::written::note_written(&glb.to_string_lossy());
    assert_eq!(
        typed("n.png"),
        Some(TextureType::Default),
        "the table dropped"
    );
    let warned = rusty_test_support::logs::during(|| {
        assert_eq!(typed("gone.png"), Some(TextureType::Default));
        assert_eq!(typed("gone.png"), Some(TextureType::Default));
    });
    assert_eq!(warned.len(), 1, "once: {warned:?}");
    assert!(warned[0].contains("gone.png"), "{warned:?}");
}
