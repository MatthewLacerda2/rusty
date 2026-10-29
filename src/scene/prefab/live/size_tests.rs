//! A list's size entry (#1527): one on a list the baseline omits (an empty list is
//! not saved) makes the list, and one past the cap is skipped, not obeyed.

use super::component_tests::{bare, play_stop};
use super::tests::save_and_load;
use crate::components::{Entity, IkConstraint, PrefabOverrides};
use crate::scene::authoring::defaults::default_animator;
use crate::scene::list_instance_overrides;
use crate::scene::prefab::overrides::apply_overrides;
use serde_json::Value;

#[test]
fn an_ik_constraint_added_where_the_source_has_none_survives_play_and_reload() {
    let (mut scene, root, child) = bare("ik_added", |s, c| {
        s.world.set_animator(c, Some(default_animator()));
    });
    let look = IkConstraint::aim("Look", vec!["Head".into()]);
    scene
        .world
        .animator_mut(child)
        .unwrap()
        .ik
        .push(look.clone());
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert_eq!(listed, ["1:/animator/ik/#size", "1:/animator/ik/0"]);

    play_stop(&mut scene);
    let back = save_and_load(&scene);
    for scene in [&scene, &back] {
        let ik = scene.world.animator(child).unwrap().ik.clone();
        assert_eq!(ik, [look.clone()]);
        assert_eq!(list_instance_overrides(scene, root).unwrap(), listed);
    }
}

#[test]
fn a_size_past_the_cap_is_skipped() {
    let mut entity = Entity::new(1, "Big".into());
    let mut overrides = PrefabOverrides::default();
    let huge = Value::from(1_u64 << 40);
    overrides
        .modifications
        .insert("/scripts/#size".into(), huge);
    overrides
        .modifications
        .insert("/name".into(), Value::from("Kept"));
    apply_overrides(&mut entity, &overrides);
    assert!(entity.scripts.is_empty(), "the list was not resized");
    assert_eq!(entity.name, "Kept", "the entity's other overrides apply");
}
