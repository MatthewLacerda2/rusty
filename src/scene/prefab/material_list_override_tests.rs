//! A prefab instance can gain a material slot its source lacks (#1285): a size entry
//! and the new slot (#1527), which apply back.

use super::apply_overrides;
use crate::components::{Entity, MaterialComponent};
use crate::scene::prefab::diff::diff_entity;

fn with_materials(keys: &[&str]) -> Entity {
    let mut e = Entity::new(3, "Quad".into());
    e.material = Some(MaterialComponent {
        materials: keys.iter().map(|k| k.to_string()).collect(),
    });
    e
}

#[test]
fn a_longer_material_list_is_a_size_and_its_new_slot() {
    let baseline = with_materials(&["Red"]);
    let instance = with_materials(&["Red", "Blue"]);
    let diff = diff_entity(&instance, &baseline);
    let keys = ["/material/materials/#size", "/material/materials/1"];
    assert_eq!(diff.keys(), keys, "{diff:?}");
    assert_eq!(diff.modifications[keys[1]], "Blue");
    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &diff);
    assert_eq!(rebuilt.material.unwrap().materials, ["Red", "Blue"]);
}

#[test]
fn a_material_added_over_none_applies_back() {
    let baseline = Entity::new(3, "Quad".into());
    let instance = with_materials(&["Gold"]);
    let mut rebuilt = baseline.clone();
    apply_overrides(&mut rebuilt, &diff_entity(&instance, &baseline));
    assert_eq!(rebuilt.material.unwrap().materials, ["Gold"]);
}
