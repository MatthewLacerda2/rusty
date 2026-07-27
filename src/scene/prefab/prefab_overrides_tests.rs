//! Tests for the generic per-instance override diff (#216).

use super::*;
use crate::components::{CameraComponent, ClearFlags, MaterialComponent};

/// A bare entity baseline with one local id.
fn base() -> Entity {
    let mut e = Entity::new(7, "Inst".into());
    e.transform.position = glam::Vec3::new(1.0, 2.0, 3.0);
    e
}

/// A camera component with distinctive scalar values (no `Default` on the type). All
/// fields are scalars, so the component round-trips through the override diff even when
/// rebuilt over a `None` baseline (the apply path does not grow JSON arrays).
fn camera() -> CameraComponent {
    CameraComponent {
        active: true,
        fov: 75.0,
        near: 0.1,
        far: 100.0,
        culling_mask: u32::MAX,
        render_order: 0,
        clear_flags: ClearFlags::Skybox,
        motion_blur_active: true,
        motion_blur_samples: 64,
        fxaa_active: false,
    }
}

#[test]
fn identical_entities_have_no_overrides() {
    let baseline = base();
    let instance = baseline.clone();
    assert!(diff_entity(&instance, &baseline).is_empty());
}

#[test]
fn a_changed_scalar_is_a_single_pointer_override() {
    let baseline = base();
    let mut instance = baseline.clone();
    instance.transform.position.x = 9.0;
    let diff = diff_entity(&instance, &baseline);
    assert_eq!(diff.len(), 1, "only the one leaf differs: {diff:?}");
    assert_eq!(
        diff.get("/transform/position/0"),
        Some(&serde_json::json!(9.0))
    );
}

#[test]
fn adding_a_component_is_an_override_at_its_path() {
    let baseline = base();
    let mut instance = baseline.clone();
    instance.camera = Some(camera());
    let diff = diff_entity(&instance, &baseline);
    // The added component surfaces as one-or-more leaves under `/camera`.
    assert!(
        diff.keys().any(|k| k.starts_with("/camera")),
        "added component appears in the diff: {diff:?}"
    );
}

#[test]
fn removing_a_component_overrides_it_to_null() {
    let mut baseline = base();
    baseline.camera = Some(camera());
    let mut instance = baseline.clone();
    instance.camera = None;
    let diff = diff_entity(&instance, &baseline);
    assert_eq!(
        diff.get("/camera"),
        Some(&serde_json::Value::Null),
        "removed component is a null override: {diff:?}"
    );
}

#[test]
fn apply_round_trips_a_diff() {
    let baseline = base();
    let mut instance = baseline.clone();
    instance.name = "Renamed".into();
    instance.transform.position.y = 42.0;
    instance.material = Some(MaterialComponent {
        material: "Gold".into(),
    });
    let diff = diff_entity(&instance, &baseline);

    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &diff);
    assert_eq!(rebuilt.name, "Renamed");
    assert_eq!(rebuilt.transform.position.y, 42.0);
    assert_eq!(rebuilt.material.as_ref().unwrap().material, "Gold");
    // Applying the diff reproduces the instance: re-diffing yields nothing.
    assert!(diff_entity(&rebuilt, &baseline) == diff_entity(&instance, &baseline));
}

#[test]
fn identity_and_geometry_are_never_overrides() {
    let mut baseline = base();
    baseline.mesh = Some(
        crate::scene::authoring::primitive_mesh_component(crate::scene::authoring::Primitive::Box)
            .unwrap(),
    );
    let mut instance = baseline.clone();
    // Different id / parent / a fatter (rehydrated) mesh must NOT be diffed.
    instance.id = 999;
    instance.parent_id = Some(123);
    if let Some(m) = instance.mesh.as_mut() {
        m.vertices.clear();
    }
    assert!(
        diff_entity(&instance, &baseline).is_empty(),
        "masked keys (id/parent/geometry) never count as overrides"
    );
}

#[test]
fn applying_an_empty_diff_is_a_noop() {
    let baseline = base();
    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &Default::default());
    assert!(diff_entity(&rebuilt, &baseline).is_empty());
}

#[test]
fn apply_can_add_a_component_over_a_none_baseline() {
    // The baseline has NO camera; the instance ADDS one. Round-tripping the diff must
    // rebuild the whole component (the override paths are created over a `null` slot).
    let baseline = base();
    let mut instance = baseline.clone();
    instance.camera = Some(camera());
    let diff = diff_entity(&instance, &baseline);

    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &diff);
    let c = rebuilt.camera.as_ref().expect("added component rebuilt");
    assert_eq!(c.fov, 75.0);
    assert_eq!(c.far, 100.0);
}

#[test]
fn apply_can_remove_a_component_via_null() {
    // The baseline HAS a camera; a `/camera -> null` override drops it.
    let mut baseline = base();
    baseline.camera = Some(camera());
    let mut overrides = std::collections::BTreeMap::new();
    overrides.insert("/camera".to_string(), serde_json::Value::Null);

    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &overrides);
    assert!(
        rebuilt.camera.is_none(),
        "null override removes the component"
    );
}
