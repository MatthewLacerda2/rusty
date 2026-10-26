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
    let bloom = scene.world.post_processing(id).unwrap().bloom.clone();
    assert_eq!(
        (bloom.active, bloom.intensity, bloom.threshold),
        (false, 0.0, 0.25)
    );
}

/// `value` at `path` inside `json`, a lane (`center.x`) indexing its array.
fn put(json: &mut serde_json::Value, path: &str, value: f32) {
    let slot = path.split('.').fold(json, |at, key| {
        let lane = ["x", "y", "z", "w"].iter().position(|l| *l == key);
        match (lane, at.is_array()) {
            (Some(i), true) => &mut at[i],
            _ => &mut at[key],
        }
    });
    *slot = value.into();
}

#[test]
fn a_load_holds_every_knob_through_the_knobs_own_setter() {
    // A knob whose setter the row's body skips would load out of range, or held by
    // another clamp than the slider's and the tween's.
    for effect in EFFECTS {
        for k in effect.knobs {
            for value in [1.0e6, -1.0e6] {
                let path = format!("{}.{}", effect.name, k.field);
                let mut json = serde_json::to_value(default_post_processing()).unwrap();
                put(&mut json, &path, value);
                let mut scene = Scene::new();
                let id = scene.add_entity("Volume".to_string());
                let loaded = serde_json::from_value(json).unwrap();
                scene.world.set_post_processing(id, Some(loaded));
                crate::scene::authoring::fields::hold_entity(&mut scene.world, id);
                let mut expected = default_post_processing();
                (k.set)(&mut expected, value);
                let held = (k.get)(&scene.world.post_processing(id).unwrap());
                assert_eq!(held, (k.get)(&expected), "{path} = {value}");
            }
        }
    }
}

/// The volume's `ssao` and `shadows` objects merge like an effect's (#1469): a
/// partial write keeps the other fields, and a misspelt key is an error.
#[test]
fn ssao_and_shadows_merge_and_refuse_unknown_keys() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".to_string());
    let mut vc = default_post_processing();
    (vc.ssao.active, vc.ssao.radius) = (false, 0.75);
    (vc.shadows.cascades, vc.shadows.distance) = (2, 40.0);
    scene.world.set_post_processing(id, Some(vc));
    let kind = find("PostProcessing").unwrap();
    let mut write = |json: serde_json::Value| {
        let changes = json.as_object().unwrap().clone();
        (kind.write)(&mut scene.world, id, &changes, Skip::Held)
    };
    assert!(
        write(serde_json::json!({"ssao": {"intensity": 2.0}, "shadows": {"distance": 90.0}}))
            .unwrap()
    );
    assert!(write(serde_json::json!({"ssao": {"intensty": 3.0}})).is_err());
    let vc = scene.world.post_processing(id).unwrap();
    assert_eq!(
        (vc.ssao.active, vc.ssao.radius, vc.ssao.intensity),
        (false, 0.75, 2.0)
    );
    assert_eq!((vc.shadows.cascades, vc.shadows.distance), (2, 90.0));
}
