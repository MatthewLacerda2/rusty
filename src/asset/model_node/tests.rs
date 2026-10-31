//! A model's node tree (#1083): world transforms down the tree, bounds with every
//! mesh placed, and the one-root-per-mesh tree of an asset with no nodes.

use glam::{Quat, Vec3};

use super::{worlds, ModelNode};
use crate::asset::{ImportedAsset, MeshVertex, SubMesh};

fn tri(id: &str) -> SubMesh {
    let v = |p: [f32; 3]| MeshVertex::new(p, [0.0, 0.0, 1.0], [0.0; 2]);
    SubMesh {
        id: id.to_string(),
        vertices: vec![v([0.0; 3]), v([1.0, 0.0, 0.0]), v([0.0, 1.0, 0.0])],
        indices: vec![0, 1, 2],
        ranges: Vec::new(),
        skin: None,
        clips: Vec::new(),
        node_name: None,
    }
}

fn node(name: &str, parent: Option<usize>, translation: Vec3, mesh: Option<&str>) -> ModelNode {
    ModelNode {
        name: name.to_string(),
        parent,
        bone: None,
        translation,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
        mesh: mesh.map(str::to_string),
    }
}

#[test]
fn worlds_compose_down_the_tree() {
    let mut child = node("Child", Some(0), Vec3::X, None);
    child.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let leaf = node("Leaf", Some(1), Vec3::X, None);
    let nodes = [
        node("Root", None, Vec3::new(0.0, 2.0, 0.0), None),
        child,
        leaf,
    ];
    let at = worlds(&nodes)[2].transform_point3(Vec3::ZERO);
    // The child's quarter turn swings the leaf's +X offset onto -Z.
    assert!(at.abs_diff_eq(Vec3::new(1.0, 2.0, -1.0), 1e-5), "{at}");
}

#[test]
fn bounds_place_every_mesh_at_its_node() {
    let asset = ImportedAsset {
        sub_meshes: vec![tri("A")],
        nodes: vec![
            node("Left", None, Vec3::new(-10.0, 0.0, 0.0), Some("A")),
            node("Right", None, Vec3::new(10.0, 0.0, 0.0), Some("A")),
            node("Empty", None, Vec3::new(0.0, 50.0, 0.0), None),
        ],
        ..Default::default()
    };
    let (lo, hi) = asset.model_bounds().unwrap();
    assert_eq!(
        (lo, hi),
        (Vec3::new(-10.0, 0.0, 0.0), Vec3::new(11.0, 1.0, 0.0))
    );
}

#[test]
fn an_asset_without_nodes_has_one_root_per_mesh() {
    let asset = ImportedAsset {
        sub_meshes: vec![tri("A"), tri("B")],
        ..Default::default()
    };
    let tree = asset.node_tree();
    assert_eq!(*tree, [ModelNode::root_of("A"), ModelNode::root_of("B")]);
    assert_eq!(
        asset.model_bounds(),
        Some((Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0)))
    );
    assert_eq!(ImportedAsset::default().model_bounds(), None);
}
