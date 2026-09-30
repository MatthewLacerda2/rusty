//! Tests for the component dependency mechanism (`dependency.rs`, #359).

use super::*;
use crate::scene::authoring::create_entity;
use crate::scene::Scene;

#[test]
fn all_kinds_are_addable_and_probeable() {
    // Guards `ComponentKind::ALL` against drift: every listed kind adds a default
    // and is then seen by the presence probe (the compiler already forces the
    // per-kind match arms to be exhaustive).
    let mut scene = Scene::new();
    for kind in ComponentKind::ALL {
        let id = create_entity(&mut scene, "E", None);
        assert!(set_default(&mut scene.world, id, kind), "{kind:?} adds");
        assert!(has_kind(&scene.world, id, kind), "{kind:?} is probeable");
    }
}

#[test]
fn ui_graphics_require_a_rect_transform() {
    let mut scene = Scene::new();
    for kind in [
        ComponentKind::Image,
        ComponentKind::Text,
        ComponentKind::RectMask,
        ComponentKind::Selectable,
        ComponentKind::LayoutGroup,
        ComponentKind::LayoutElement,
    ] {
        let id = create_entity(&mut scene, "E", None);
        assert!(add_with_requirements(&mut scene.world, id, kind));
        assert!(scene.world.has_rect_transform(id), "{kind:?} brings a rect");
    }
}

#[test]
fn add_auto_satisfies_requirement() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "FX", None);
    assert!(add_with_requirements(
        &mut scene.world,
        id,
        ComponentKind::VisualCorrection
    ));
    assert!(scene.world.has_visual_correction(id));
    assert!(scene.world.has_camera(id), "requirement auto-added");
}

#[test]
fn remove_cascades_to_dependents() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "FX", None);
    add_with_requirements(&mut scene.world, id, ComponentKind::VisualCorrection);
    assert!(remove_with_cascade(
        &mut scene.world,
        id,
        ComponentKind::Camera
    ));
    assert!(!scene.world.has_camera(id));
    assert!(!scene.world.has_visual_correction(id), "dependent cascaded");
}

#[test]
fn reconcile_drops_orphaned_dependent() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "FX", None);
    add_with_requirements(&mut scene.world, id, ComponentKind::VisualCorrection);
    // Clear the requirement out from under the dependent (as another card might).
    scene.world.set_camera(id, None);
    assert!(reconcile_requirements(&mut scene.world, id));
    assert!(!scene.world.has_visual_correction(id));
}

#[test]
fn enforce_on_load_auto_adds_missing_requirement() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "FX", None);
    // Simulate a hand-edited scene: a dependent with no requirement.
    scene
        .world
        .set_visual_correction(id, Some(default_visual_correction()));
    let added = enforce_requirements(&mut scene.world, id);
    assert_eq!(
        added,
        vec![(ComponentKind::VisualCorrection, ComponentKind::Camera)]
    );
    assert!(scene.world.has_camera(id));
    // Idempotent: a second pass finds nothing to add.
    assert!(enforce_requirements(&mut scene.world, id).is_empty());
}
