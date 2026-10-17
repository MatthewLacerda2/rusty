//! A card's Remove button (#992): the cascading verb behind `Scene.RemoveComponent`,
//! so a dependent leaves with what it requires, and the scene is marked unsaved.

use super::remove_component;
use crate::scene::authoring::{add_with_requirements, has_kind, ComponentKind};
use crate::scene::Scene;

#[test]
fn removing_a_required_component_takes_its_dependents_and_dirties_the_scene() {
    let mut scene = Scene::new();
    let id = scene.add_entity("label".to_string());
    assert!(add_with_requirements(
        &mut scene.world,
        id,
        ComponentKind::Text
    ));
    assert!(has_kind(&scene.world, id, ComponentKind::RectTransform));
    let mut dirty = false;
    remove_component(
        &mut scene.world,
        id,
        ComponentKind::RectTransform,
        &mut dirty,
    );
    assert!(!has_kind(&scene.world, id, ComponentKind::Text));
    assert!(!has_kind(&scene.world, id, ComponentKind::RectTransform));
    assert!(dirty);
}
