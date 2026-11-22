use super::*;
use crate::create::{primitive_mesh_component, Primitive};
use rusty_components::registry::default_drift;

// Exactly the rows `components`' `registry_tests::drift_of!` skips: change the two lists
// together, or a skipped row is checked nowhere.
#[test]
fn a_missing_key_loads_the_add_component_default() {
    let mesh = primitive_mesh_component(Primitive::Box).expect("a box mesh");
    let drift: Vec<String> = [
        default_drift(&default_collider()),
        default_drift(&default_light()),
        default_drift(&MaterialComponent::for_entity(7)),
        default_drift(&mesh),
        default_drift(&default_rigidbody()),
    ]
    .concat();
    assert!(
        drift.is_empty(),
        "a serde default differs from the Add Component default, so a scene missing \
         that key loads another value; make the serde default return it, or give \
         the key a `FirstClass::DEFAULT_EXCEPTIONS` entry saying why: {drift:#?}"
    );
}
