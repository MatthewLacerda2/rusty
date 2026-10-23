//! A saved scene names an imported model's extracted textures by reference
//! (`model.fbx#tex/<identity>`), never by their path in the import cache (#1130).

use glam::Vec3;

use super::instantiate_asset;
use crate::asset::convert::texture_ref;
use crate::scene::{apply_scene_data, to_scene_data, Scene, SceneData};

const FIXTURE: &str = include_str!("../../../tests/fixtures/models/embedded_textures.fbx");

/// A texture of the same file name as Red's (`skin.png`), on a material listed
/// before Red, holding Blue's pixels: converted first, it takes `skin.png` and
/// pushes Red's file to `skin.1.png`.
const GREEN: &str = r#"	Material: 2900, "Material::Green", "" {
		Version: 102
		ShadingModel: "lambert"
	}
	Video: 3900, "Video::Green", "Clip" {
		Type: "Clip"
		Filename: "/missing/skin.png"
		RelativeFilename: "skin.png"
		Content: , "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNgYPgPAAEDAQAIicLsAAAAAElFTkSuQmCC"
	}
	Texture: 3950, "Texture::Green", "" {
		Type: "TextureVideoClip"
		Version: 202
		TextureName: "Texture::Green"
		FileName: "/missing/skin.png"
		RelativeFilename: "skin.png"
	}
"#;

/// `dir/quad.fbx`, the fixture, with [`GREEN`] inserted before Red when asked.
fn write_quad(dir: &std::path::Path, green: bool) -> String {
    let mut text = FIXTURE.to_string();
    if green {
        let red = text.find("\tMaterial: 3000").expect("Red's material");
        text.insert_str(red, GREEN);
        let link = "\tC: \"OO\",3900,3950\n\tC: \"OP\",3950,2900, \"DiffuseColor\"\n";
        let connections = text.find("\tC: ").expect("a connection");
        text.insert_str(connections, link);
    }
    let path = dir.join("quad.fbx");
    std::fs::write(&path, text).unwrap();
    path.to_string_lossy().into_owned()
}

/// A scene holding `node` of the model at `path`, and the key of its material.
fn instanced(path: &str, node: &str) -> (Scene, String) {
    let mut scene = Scene::new();
    let id = instantiate_asset(&mut scene, &format!("{path}::{node}"), None, Vec3::ZERO).unwrap();
    let key = scene
        .world
        .material(id)
        .expect("a material")
        .material
        .clone();
    (scene, key)
}

fn base_color_map(scene: &Scene, key: &str) -> String {
    scene.materials[key]
        .base_color_map
        .as_ref()
        .expect("a map")
        .to_string()
}

#[test]
fn a_saved_scene_never_names_the_import_cache() {
    let dir = crate::test_temp::dir();
    let path = write_quad(&dir, false);
    let (scene, key) = instanced(&path, "Quad");
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(!json.contains("generated/imported"), "{json}");
    assert!(!json.contains(".textures/"), "{json}");
    assert_eq!(
        base_color_map(&scene, &key),
        texture_ref::reference(&path, "Red")
    );
}

#[test]
fn a_texture_on_disk_beside_the_fbx_is_saved_as_its_own_path() {
    let dir = crate::test_temp::dir();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/models/missing_texture.fbx"
    );
    let path = dir.join("tri.fbx");
    std::fs::copy(fixture, &path).unwrap();
    // The FBX names it `gone.png`, relative to itself.
    std::fs::write(dir.join("gone.png"), b"\x89PNG").unwrap();
    let (scene, key) = instanced(&path.to_string_lossy(), "Tri");
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    assert!(!json.contains("generated/imported"), "{json}");
    assert!(!json.contains(".."), "{json}");
    let map = base_color_map(&scene, &key);
    assert_eq!(std::fs::read(map).unwrap(), b"\x89PNG", "the file itself");
}

#[test]
fn a_reconversion_inserting_a_same_named_texture_keeps_each_map_its_own() {
    let dir = crate::test_temp::dir();
    let path = write_quad(&dir, false);
    let (scene, key) = instanced(&path, "Quad");
    let map = base_color_map(&scene, &key);
    let red = std::fs::read(texture_ref::file(&map).expect("resolves")).unwrap();

    // The edit, then a scene load: the load re-imports the model.
    write_quad(&dir, true);
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, to_scene_data(&scene));
    assert_eq!(base_color_map(&restored, &key), map);
    let file = texture_ref::file(&map).expect("still resolves");
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(name, "skin.1.png", "the edit moved Red's file");
    assert_eq!(std::fs::read(file).unwrap(), red, "Red's own pixels");
}

#[test]
fn a_scene_with_a_texture_reference_round_trips() {
    let dir = crate::test_temp::dir();
    let (scene, key) = instanced(&write_quad(&dir, false), "Quad");
    let json = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    let data: SceneData = serde_json::from_str(&json).unwrap();
    let mut restored = Scene::new();
    apply_scene_data(&mut restored, data);
    assert_eq!(
        base_color_map(&restored, &key),
        base_color_map(&scene, &key)
    );
    let again = serde_json::to_string(&to_scene_data(&restored)).unwrap();
    assert_eq!(again, json);
}
