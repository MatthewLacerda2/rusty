//! The GLB writer (#1024): what it writes parses as glTF and carries what it was given.

use super::{write, Document, Material, Mesh, Node, Primitive, Skin};

fn triangle() -> Document {
    let mut root = Node::new("Tri");
    root.mesh = Some(0);
    root.skin = Some(0);
    let primitive = Primitive {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        joints: vec![[0; 4]; 3],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        material: Some(0),
    };
    let texture = Some("../textures/a b.png".to_string());
    Document {
        nodes: vec![root],
        roots: vec![0],
        meshes: vec![Mesh {
            name: "Tri".into(),
            primitives: vec![primitive],
        }],
        materials: vec![Material {
            name: "M".into(),
            base_color_texture: texture.clone(),
            normal_texture: texture,
            ..Material::default()
        }],
        skins: vec![Skin {
            joints: vec![0],
            inverse_bind: vec![glam::Mat4::IDENTITY.to_cols_array()],
        }],
    }
}

#[test]
fn a_written_triangle_reads_back() {
    let glb = gltf::Gltf::from_slice(&write(&triangle())).expect("valid glb");
    let mesh = glb.meshes().next().unwrap();
    assert_eq!(mesh.name(), Some("Tri"));
    let primitive = mesh.primitives().next().unwrap();
    let position = primitive.get(&gltf::Semantic::Positions).unwrap();
    assert_eq!(position.count(), 3);
    assert_eq!(position.max(), Some(serde_json::json!([1.0, 2.0, 0.0])));
    assert!(primitive.get(&gltf::Semantic::Joints(0)).is_some());
    assert_eq!(glb.skins().count(), 1);
}

#[test]
fn one_texture_used_twice_is_one_image() {
    let glb = gltf::Gltf::from_slice(&write(&triangle())).unwrap();
    assert_eq!(glb.images().count(), 1);
    let material = glb.materials().next().unwrap();
    assert!(material.normal_texture().is_some());
}

#[test]
fn an_empty_document_is_still_a_valid_glb() {
    let mut doc = Document::default();
    doc.nodes.push(Node::new("Empty"));
    doc.roots.push(0);
    let glb = gltf::Gltf::from_slice(&write(&doc)).expect("valid glb");
    assert_eq!(glb.nodes().count(), 1);
    assert!(glb.blob.is_none());
}
