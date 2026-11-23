//! A card's Remove button: the cascading verb behind `Scene.RemoveComponent`,
//! so a dependent leaves with what it requires.

use super::remove_component;
use rusty_edit::authoring::{add_with_requirements, has_kind, ComponentKind};
use rusty_scene::Scene;

#[test]
fn removing_a_required_component_takes_its_dependents() {
    let mut scene = Scene::new();
    let id = scene.add_entity("label".to_string());
    assert!(add_with_requirements(
        &mut scene.world,
        id,
        ComponentKind::Text
    ));
    assert!(has_kind(&scene.world, id, ComponentKind::RectTransform));
    remove_component(&mut scene.world, id, ComponentKind::RectTransform);
    assert!(!has_kind(&scene.world, id, ComponentKind::Text));
    assert!(!has_kind(&scene.world, id, ComponentKind::RectTransform));
}
