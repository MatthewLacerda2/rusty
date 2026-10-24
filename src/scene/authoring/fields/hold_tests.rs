//! A loaded scene is held through the field table's ops (#1418): no first-class
//! component's numeric field skips the pass, and an in-range file loads unchanged.

use serde_json::Value;

use super::hold::is_numeric;
use super::tests::dressed;
use super::{Skip, KINDS};
use crate::scene::{apply_scene_data, to_scene_data, Scene};

/// Every float in `v` pushed far out of any bound: `sign` 1 up, -1 down.
fn push_out(v: &mut Value, sign: f64) {
    match v {
        Value::Number(n) if n.is_f64() => {
            let far = sign * (n.as_f64().unwrap().abs() * 1000.0 + 1000.0);
            *v = serde_json::json!(far);
        }
        Value::Array(list) => list.iter_mut().for_each(|v| push_out(v, sign)),
        Value::Object(map) => map.values_mut().for_each(|v| push_out(v, sign)),
        _ => {}
    }
}

/// The same numbers, give or take float rounding (renormalizing a unit axis
/// moves it by an ulp).
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            (a - b).abs() <= 1e-5 * a.abs().max(1.0)
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| close(v, w)))
        }
        _ => a == b,
    }
}

/// `dressed()` saved with every writable numeric field pushed out, and loaded.
fn load_pushed(sign: f64) -> (Scene, u32) {
    let (scene, id) = dressed();
    let mut doc = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let entity = doc["entities"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == id)
        .unwrap();
    for kind in KINDS {
        for (name, read_only) in (kind.fields)() {
            let field = entity.get_mut(kind.key).and_then(|c| c.get_mut(name));
            if let Some(field) = field.filter(|f| read_only.is_none() && is_numeric(f)) {
                push_out(field, sign);
            }
        }
    }
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_value(doc).unwrap());
    (loaded, id)
}

/// The walk: after a load, every writable numeric field of every first-class
/// component holds a value its own op keeps as is, so a field added to a table
/// is held on load with nothing more to write.
#[test]
fn every_numeric_field_loads_as_its_op_would_hold_it() {
    for sign in [1.0, -1.0] {
        let (mut scene, id) = load_pushed(sign);
        for kind in KINDS {
            let held = (kind.read)(&scene.world, id).unwrap().unwrap();
            for (name, read_only) in (kind.fields)() {
                if read_only.is_some() || !is_numeric(&held[name]) {
                    continue;
                }
                let one = serde_json::Map::from_iter([(name.to_string(), held[name].clone())]);
                let written = (kind.write)(&mut scene.world, id, &one, Skip::Nothing);
                assert_eq!(written, Ok(true), "{}.{name}", kind.name);
                let again = (kind.read)(&scene.world, id).unwrap().unwrap();
                assert!(
                    close(&again[name], &held[name]),
                    "{}.{name} loaded as {} (sign {sign}), which its op moves to {}",
                    kind.name,
                    held[name],
                    again[name]
                );
            }
        }
    }
}

/// The issue's cases: a loaded `volume: 4` plays at 1, a `CanvasGroup.alpha: 3` at 1.
#[test]
fn out_of_range_values_are_held_on_load() {
    let (scene, id) = load_pushed(1.0);
    assert_eq!(scene.world.audio(id).unwrap().volume, 1.0);
    assert_eq!(scene.world.canvas_group(id).unwrap().alpha, 1.0);
    let (scene, id) = load_pushed(-1.0);
    assert_eq!(scene.world.audio(id).unwrap().volume, 0.0);
    assert_eq!(scene.world.audio(id).unwrap().final_distance, 0.0);
}

/// A file already in range loads exactly as saved: no op's side effect (a reverb's
/// `params` making its preset `Custom`) runs on a value the ops would keep.
#[test]
fn an_in_range_scene_loads_unchanged() {
    let (mut scene, id) = dressed();
    let hall = crate::components::ReverbPreset::Hall;
    crate::scene::authoring::reverb_zone::set_preset(
        &mut scene.world.reverb_zone_mut(id).unwrap(),
        hall,
    );
    let saved = serde_json::to_value(to_scene_data(&scene)).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_value(saved.clone()).unwrap());
    assert_eq!(serde_json::to_value(to_scene_data(&loaded)).unwrap(), saved);
}

/// A prefab file is held as a scene file is.
#[test]
fn an_instantiated_prefab_is_held() {
    let (mut scene, id) = dressed();
    scene.world.audio_mut(id).unwrap().volume = 4.0;
    let prefab = crate::scene::prefab::extract_prefab(&scene, id).unwrap();
    let root = crate::scene::prefab::instantiate_prefab(&mut scene, &prefab, None);
    assert_eq!(scene.world.audio(root).unwrap().volume, 1.0);
}
