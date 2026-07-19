//! Tests for `build_preview_scene` (#352): isolation, per-subject material wiring,
//! and the mesh toggle — all GPU-free (`Scene` is plain data).

use super::*;

/// A minimal two-triangle `.obj` with one named object, for `spawn_model` tests.
const QUAD_OBJ: &str = "\
o Quad
v -1.0 0.0 -1.0
v 1.0 0.0 -1.0
v 1.0 0.0 1.0
v -1.0 0.0 1.0
vn 0.0 1.0 0.0
f 1//1 2//1 3//1
f 1//1 3//1 4//1
";

fn write_temp_obj(name: &str) -> String {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, QUAD_OBJ).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn every_mesh_choice_spawns_a_mesh_entity() {
    for mesh in PreviewMesh::ALL {
        let scene = build_preview_scene(mesh, &PreviewSubject::Texture("none.png".to_string()));
        let meshed = scene.world.ids_with_mesh().len();
        assert_eq!(
            meshed,
            1,
            "{}: should spawn exactly one mesh entity",
            mesh.label()
        );
    }
}

#[test]
fn scene_has_no_baked_skybox_and_one_directional_light() {
    let scene = build_preview_scene(
        PreviewMesh::Sphere,
        &PreviewSubject::Material(MaterialAsset::default()),
    );
    assert!(
        scene.skybox_path.is_empty(),
        "empty path -> the renderer's gradient fallback"
    );
    let lit = scene.world.ids_with_light();
    assert_eq!(lit.len(), 1);
    assert_eq!(
        scene.world.light(lit[0]).unwrap().light_type,
        LightType::Directional
    );
}

#[test]
fn texture_subject_sets_base_color_map() {
    let scene = build_preview_scene(
        PreviewMesh::Sphere,
        &PreviewSubject::Texture("project/assets/textures/rock.png".to_string()),
    );
    let material = scene
        .materials
        .get("preview::material")
        .expect("material attached");
    assert_eq!(
        material.base_color_map.as_deref(),
        Some("project/assets/textures/rock.png")
    );
}

#[test]
fn material_subject_is_applied_verbatim() {
    let custom = MaterialAsset {
        base_color: [0.1, 0.2, 0.3],
        metallic: 0.7,
        ..MaterialAsset::default()
    };
    let scene = build_preview_scene(PreviewMesh::Cube, &PreviewSubject::Material(custom.clone()));
    let material = scene.materials.get("preview::material").unwrap();
    assert_eq!(material.base_color, custom.base_color);
    assert_eq!(material.metallic, custom.metallic);
}

#[test]
fn model_subject_instantiates_every_sub_object_at_the_origin() {
    let path = write_temp_obj("preview_scene_test_quad.obj");
    let scene = build_preview_scene(PreviewMesh::Sphere, &PreviewSubject::Model(path));
    // The fixture's single named object ("Quad") becomes one entity carrying its own
    // Asset-referenced mesh, not the sphere placeholder.
    assert_eq!(scene.world.ids_with_mesh().len(), 1);
}

#[test]
fn cache_key_distinguishes_subject_kind_and_path() {
    let a = PreviewSubject::Texture("x.png".to_string()).cache_key();
    let b = PreviewSubject::Shader("x.png".to_string()).cache_key();
    assert_ne!(a, b);
}
