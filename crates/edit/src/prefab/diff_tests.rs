use super::masked;
use crate::authoring::{add_component, create_entity, ComponentKind, Primitive};
use rusty_scene::Scene;

#[test]
fn the_reference_document_masks_like_the_whole_one() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Dressed", Some(Primitive::Box));
    for &kind in ComponentKind::ALL {
        add_component(&mut scene, id, kind);
    }
    let mut mesh = scene.world.mesh_mut(id).expect("a mesh");
    mesh.primitive_type = "Plane".into();
    mesh.subdivisions = 4;
    drop(mesh);
    let whole = masked(&scene.world.entity_document(id).unwrap());
    let reference = masked(&scene.world.reference_document(id).unwrap());
    assert_eq!(reference, whole);
    assert_eq!(whole["mesh"]["subdivisions"], 4);
}
