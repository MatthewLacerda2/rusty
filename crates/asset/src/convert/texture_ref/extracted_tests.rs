//! An extracted texture's import settings by the slots its glTF samples it in,
//! and which images count as extracted (#2365).

use super::extracted::{glb_of, model_source, of_slots, settings};
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
