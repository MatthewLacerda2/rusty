//! The authoring registry (#1467): every effect's row reaches the field table, its
//! knobs resolve by path, and a partial settings table keeps what it leaves out.

use super::*;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::fields::{find, Skip};
use crate::scene::Scene;

#[test]
fn the_field_table_holds_the_settings_then_every_effect_in_order() {
    let names: Vec<&str> = FIELDS.iter().map(|f| f.name).collect();
    let effects: Vec<&str> = EFFECTS.iter().map(|e| e.name).collect();
    assert_eq!(names[..3], PostProcessingComponent::NOT_EFFECTS);
    assert_eq!(names[3..], effects[..]);
}

#[test]
fn every_knob_resolves_by_its_path_and_reads_what_it_writes() {
    for effect in EFFECTS {
        for k in effect.knobs {
            let path = format!("{}.{}", effect.name, k.field);
            let found = knob(&path).unwrap_or_else(|| panic!("{path}"));
            assert_eq!(found.field, k.field, "{path}");
            // Near its default, so inside its range (and its pair's).
            let mut vc = default_post_processing();
            let value = (k.get)(&vc) * 0.9 + 0.05;
            (k.set)(&mut vc, value);
            assert_eq!((k.get)(&vc), value, "{path}");
        }
    }
    assert!(knob("bloom.active").is_none(), "numbers only");
    assert!(knob("no_such_effect.intensity").is_none());
}

#[test]
fn a_partial_settings_table_keeps_the_fields_it_leaves_out() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".to_string());
    let mut vc = default_post_processing();
    (vc.bloom.active, vc.bloom.threshold) = (false, 0.25);
    scene.world.set_post_processing(id, Some(vc));
    let kind = find("PostProcessing").unwrap();
    let changes = serde_json::json!({"bloom": {"intensity": -2.0}});
    let changes = changes.as_object().unwrap();
    assert!((kind.write)(&mut scene.world, id, changes, Skip::Held).unwrap());
    let bloom = scene.world.post_processing(id).unwrap().bloom;
    assert_eq!(
        (bloom.active, bloom.intensity, bloom.threshold),
        (false, 0.0, 0.25)
    );
}
