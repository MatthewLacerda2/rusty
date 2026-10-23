//! An instance's material list is one override (#1285): a prefab instance can gain
//! a slot its source lacks, and the diff applies back.

use super::{apply_overrides, diff_entity};
use crate::components::{Entity, MaterialComponent};

fn with_materials(keys: &[&str]) -> Entity {
    let mut e = Entity::new(3, "Quad".into());
    e.material = Some(MaterialComponent {
        materials: keys.iter().map(|k| k.to_string()).collect(),
    });
    e
}

#[test]
fn a_longer_material_list_is_one_leaf_that_applies_back() {
    let baseline = with_materials(&["Red"]);
    let instance = with_materials(&["Red", "Blue"]);
    let diff = diff_entity(&instance, &baseline);
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(
        diff["/material/materials"],
        serde_json::json!(["Red", "Blue"])
    );
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
