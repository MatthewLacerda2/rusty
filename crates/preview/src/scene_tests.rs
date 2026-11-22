use super::*;
use rusty_asset::MapRef;

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

const OFFSET_QUAD_OBJ: &str = "\
o Quad
v 9.0 5.0 -1.0
v 11.0 5.0 -1.0
v 11.0 5.0 1.0
v 9.0 5.0 1.0
vn 0.0 1.0 0.0
f 1//1 2//1 3//1
f 1//1 3//1 4//1
";

/// The quad a hundredth of the size: a 2 cm square.
const TINY_QUAD_OBJ: &str = "\
o Quad
v -0.01 0.0 -0.01
v 0.01 0.0 -0.01
v 0.01 0.0 0.01
v -0.01 0.0 0.01
vn 0.0 1.0 0.0
f 1//1 2//1 3//1
f 1//1 3//1 4//1
";

/// Two parts over x 8..12, neither at the union centre: each recentred alone,
/// they stack.
const TWO_PART_OBJ: &str = "\
o Left
v 8.0 -1.0 -1.0
v 9.0 -1.0 -1.0
v 9.0 1.0 1.0
vn 0.0 1.0 0.0
f 1//1 2//1 3//1
o Right
v 11.0 -1.0 -1.0
v 12.0 -1.0 -1.0
v 12.0 1.0 1.0
f 4//1 5//1 6//1
";

fn write_temp_obj(name: &str) -> String {
    write_obj(name, QUAD_OBJ)
}

fn write_obj(name: &str, contents: &str) -> String {
    let path = rusty_test_support::temp::dir().join(name);
    std::fs::write(&path, contents).unwrap();
    path.to_string_lossy().into_owned()
}

fn subject(scene: &Scene, id: u32) -> u32 {
    scene.world.ancestors(id).last().unwrap_or(id)
}

fn mesh_position(scene: &Scene) -> Vec3 {
    let id = subject(scene, scene.world.ids_with_mesh()[0]);
    scene.world.transform(id).unwrap().position
}

#[test]
fn every_mesh_choice_spawns_a_mesh_entity() {
    for mesh in PreviewMesh::ALL {
        let scene =
            build_preview_scene(mesh, None, &PreviewSubject::Texture("none.png".to_string()))
                .unwrap();
        let meshed = scene.world.ids_with_mesh().len();
        assert_eq!(
            meshed,
            1,
            "{}: should spawn exactly one mesh entity",
            mesh.name()
        );
    }
}

#[test]
fn scene_has_no_baked_skybox_and_one_directional_light() {
    let scene = build_preview_scene(
        PreviewMesh::Sphere,
        None,
        &PreviewSubject::Material(Box::default()),
    )
    .unwrap();
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
        None,
        &PreviewSubject::Texture("assets/textures/rock.png".to_string()),
    )
    .unwrap();
    let material = scene
        .materials
        .get("preview::material")
        .expect("material attached");
    assert_eq!(
        material.base_color_map.as_ref().map(MapRef::as_str),
        Some("assets/textures/rock.png")
    );
}

#[test]
fn material_subject_is_applied_verbatim() {
    let custom = MaterialAsset {
        base_color: [0.1, 0.2, 0.3],
        metallic: 0.7,
        ..MaterialAsset::default()
    };
    let scene = build_preview_scene(
        PreviewMesh::Cube,
        None,
        &PreviewSubject::Material(Box::new(custom.clone())),
    )
    .unwrap();
    let material = scene.materials.get("preview::material").unwrap();
    assert_eq!(material.base_color, custom.base_color);
    assert_eq!(material.metallic, custom.metallic);
}

#[test]
fn model_subject_instantiates_every_sub_object_at_the_origin() {
    let path = write_temp_obj("preview_scene_test_quad.obj");
    let scene =
        build_preview_scene(PreviewMesh::Sphere, None, &PreviewSubject::Model(path)).unwrap();
    // The fixture's single named object ("Quad") becomes one entity carrying its own
    // Asset-referenced mesh, not the sphere placeholder.
    assert_eq!(scene.world.ids_with_mesh().len(), 1);
}

#[test]
fn an_off_origin_model_is_recentred_where_the_camera_looks() {
    let path = write_obj("preview_scene_test_offset.obj", OFFSET_QUAD_OBJ);
    let scene =
        build_preview_scene(PreviewMesh::Sphere, None, &PreviewSubject::Model(path)).unwrap();
    let scale = fit_scale(Vec3::new(9.0, 5.0, -1.0), Vec3::new(11.0, 5.0, 1.0));
    let expected = -Vec3::new(10.0, 5.0, 0.0) * scale;
    assert!(
        (mesh_position(&scene) - expected).length() < 1e-3,
        "expected the subject centred on the origin, got {:?}",
        mesh_position(&scene)
    );
}

#[test]
fn a_multi_part_model_shares_one_offset_so_its_layout_survives() {
    let path = write_obj("preview_scene_test_two_part.obj", TWO_PART_OBJ);
    let scene =
        build_preview_scene(PreviewMesh::Sphere, None, &PreviewSubject::Model(path)).unwrap();

    let meshes = scene.world.ids_with_mesh();
    assert_eq!(meshes.len(), 2, "both sub-objects spawn");
    let root = subject(&scene, meshes[0]);
    assert_eq!(root, subject(&scene, meshes[1]), "under one root");
    let scale = fit_scale(Vec3::new(8.0, -1.0, -1.0), Vec3::new(12.0, 1.0, 1.0));
    let position = scene.world.transform(root).unwrap().position;
    assert!(
        (position - Vec3::new(-10.0, 0.0, 0.0) * scale).length() < 1e-3,
        "the root carries the union offset, got {position:?}"
    );
}

#[test]
fn a_centimetre_model_is_scaled_up_to_fill_the_frame() {
    // An FBX in its default unit, the centimetre, imports a hundredth of a metre
    // across: at the fixed camera distance it was a sub-pixel speck.
    let path = write_obj("preview_scene_test_tiny.obj", TINY_QUAD_OBJ);
    let scene =
        build_preview_scene(PreviewMesh::Sphere, None, &PreviewSubject::Model(path)).unwrap();
    let id = subject(&scene, scene.world.ids_with_mesh()[0]);
    let scale = scene.world.transform(id).unwrap().scale;
    let radius = Vec3::new(0.02, 0.0, 0.02).length() * 0.5 * scale.x;
    assert!((radius - FIT_RADIUS).abs() < 1e-4, "fitted radius {radius}");
    assert_eq!(scale, Vec3::splat(scale.x), "the scale is uniform");
}

#[test]
fn a_point_keeps_its_size_and_a_flat_quad_is_fitted() {
    assert_eq!(fit_scale(Vec3::ONE, Vec3::ONE), 1.0);
    let unit = fit_scale(Vec3::splat(-0.5), Vec3::splat(0.5));
    assert!(
        (unit - 1.0).abs() < 1e-5,
        "a unit cube previews at its size"
    );
    let quad = fit_scale(Vec3::new(-0.01, 0.0, -0.01), Vec3::new(0.01, 0.0, 0.01));
    assert!(quad > 1.0, "flat in one axis is still fitted: {quad}");
}

#[test]
fn the_bundled_suzanne_is_pulled_into_frame() {
    let scene = build_preview_scene(
        PreviewMesh::Suzanne,
        None,
        &PreviewSubject::Shader("unused.wgsl".to_string()),
    )
    .unwrap();
    let position = mesh_position(&scene);
    assert!(
        position.length() > 1.0 && position.is_finite(),
        "Suzanne needs a real recentring offset, got {position:?}"
    );
}

#[test]
fn the_bundled_suzanne_is_fitted_like_a_model() {
    let scene = build_preview_scene(
        PreviewMesh::Suzanne,
        None,
        &PreviewSubject::Shader("unused.wgsl".to_string()),
    )
    .unwrap();
    let asset = rusty_asset::import_and_sync_sidecar(Path::new(&suzanne_path())).unwrap();
    let (min, max) = asset.sub_meshes[0].bounds().unwrap();
    let id = scene.world.ids_with_mesh()[0];
    let transform = scene.world.transform(id).unwrap();
    let scale = fit_scale(min, max);
    assert_eq!(transform.scale, Vec3::splat(scale));
    // Centred after scaling: its bounds' centre lands on the world origin.
    assert_eq!(transform.position, -(min + max) * 0.5 * scale);
}

#[test]
fn cache_key_distinguishes_subject_kind_and_path() {
    let a = PreviewSubject::Texture("x.png".to_string()).cache_key();
    let b = PreviewSubject::Shader("x.png".to_string()).cache_key();
    assert_ne!(a, b);
}
