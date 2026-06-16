//! The `"Asset"` mesh-reference scene variant (issue #74): an imported sub-mesh is
//! stored as a path-based reference (`path::sub_object`) and rehydrated by
//! RE-IMPORTING the source file on load — never by inlining GPU buffers.

use rusty::scene::Scene;

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

fn tmp(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(name);
    p
}

#[test]
fn asset_mesh_reference_rehydrates_on_load() {
    let obj = tmp("rusty_ref_quad.obj");
    std::fs::write(&obj, QUAD_OBJ).unwrap();
    let reference = format!("{}::Quad", obj.to_string_lossy());

    let mut scene = Scene::new();
    let id = scene.add_entity("Quad".to_string());
    scene.get_entity_mut(id).unwrap().mesh = Some(rusty::scene::asset_mesh_component(&reference));

    // The component imported geometry up front.
    assert_eq!(
        scene
            .get_entity(id)
            .unwrap()
            .mesh
            .as_ref()
            .unwrap()
            .vertices
            .len(),
        4
    );

    let path = tmp("rusty_asset_ref.scene");
    scene.save_to_file(path.to_str().unwrap()).unwrap();

    // The on-disk document stores the REFERENCE, never the vertices.
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains("\"primitive_type\": \"Asset\""));
    assert!(json.contains(&reference));
    assert!(!json.contains("\"vertices\""));

    // Loading re-imports the file and rebuilds the geometry from the reference.
    let mut loaded = Scene::new();
    loaded.load_from_file(path.to_str().unwrap()).unwrap();
    let mesh = loaded.get_entity(id).unwrap().mesh.clone().unwrap();
    assert_eq!(mesh.primitive_type, "Asset");
    assert_eq!(mesh.asset_ref.as_deref(), Some(reference.as_str()));
    assert_eq!(mesh.vertices.len(), 4);
    assert_eq!(mesh.indices.len(), 6);
}
