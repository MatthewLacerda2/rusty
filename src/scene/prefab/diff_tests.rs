//! The diff reads a mesh's reference without its geometry (#1527), and loses
//! nothing serde saves: every saved field of every component is in it.

use super::masked;
use crate::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use crate::scene::Scene;

#[test]
fn the_reference_document_masks_like_the_whole_one() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Dressed", Some(Primitive::Box));
    for &kind in ComponentKind::ALL {
        add_component(&mut scene, id, kind);
    }
    // A saved field `reference_only` once dropped (a subdivided Plane).
    let mut mesh = scene.world.mesh_mut(id).expect("a mesh");
    mesh.primitive_type = "Plane".into();
    mesh.subdivisions = 4;
    drop(mesh);
    let whole = masked(&scene.world.entity_document(id).unwrap());
    let reference = masked(&scene.world.reference_document(id).unwrap());
    assert_eq!(reference, whole);
    assert_eq!(whole["mesh"]["subdivisions"], 4);
}
